import { QdrantClient } from "@qdrant/js-client-rest";
import { settings } from "../config/settings";
import { logger } from "../utils/logger";

export interface Tier2Event {
	textForEmbedding: string;
	payload: Record<string, unknown>;
}

export interface QdrantScoredPoint {
	id: string | number;
	score: number;
	payload?: Record<string, unknown>;
	version?: number;
}

type PayloadIndexFieldSchema = Parameters<
	QdrantClient["createPayloadIndex"]
>[1]["field_schema"];

export class QdrantService {
	private client: QdrantClient;
	private tier2Dim: number = 64;
	private tier2CollectionPrefix: string = "via_forensic_index_v2";
	private embeddingBaseUrl: string;
	private embeddingModel: string;
	private embeddingDimension: number;
	private embeddingUnavailableUntilMs = 0;
	private embeddingDegradedLogged = false;
	private embeddingCache = new Map<
		string,
		{ vector: number[]; expiresAt: number }
	>();

	constructor() {
		this.client = new QdrantClient({
			url: `http://${settings.qdrant.host}:${settings.qdrant.port}`,
		});
		const legacyUrl = process.env.EMBEDDING_URL;
		this.embeddingBaseUrl =
			process.env.EMBEDDING_BASE_URL ||
			(legacyUrl ? legacyUrl.replace(/\/embeddings$/, "") : "") ||
			"http://127.0.0.1:1234/v1";
		this.embeddingModel =
			process.env.EMBEDDING_MODEL || "text-embedding-nomic-embed-text-v1.5";
		this.embeddingDimension = Number.parseInt(
			process.env.EMBEDDING_DIMENSION || "64",
			10,
		);
		this.tier2Dim = this.embeddingDimension;
	}

	private async mapWithConcurrency<T, R>(
		items: T[],
		limit: number,
		mapper: (item: T, index: number) => Promise<R>,
	): Promise<R[]> {
		const capped = Math.max(1, limit);
		const results: R[] = new Array(items.length);
		let cursor = 0;
		const workers = Array.from(
			{ length: Math.min(capped, items.length) },
			async () => {
				while (true) {
					const idx = cursor;
					cursor += 1;
					if (idx >= items.length) {
						break;
					}
					results[idx] = await mapper(items[idx], idx);
				}
			},
		);
		await Promise.all(workers);
		return results;
	}

	private normalizeEmbeddingText(text: string): string {
		return text.trim().replace(/\s+/g, " ").toLowerCase();
	}

	private getCachedEmbedding(text: string): number[] | null {
		const key = Bun.hash
			.xxHash64(this.normalizeEmbeddingText(text))
			.toString(16);
		const cached = this.embeddingCache.get(key);
		const now = Math.floor(Date.now() / 1000);
		if (!cached) {
			return null;
		}
		if (cached.expiresAt <= now) {
			this.embeddingCache.delete(key);
			return null;
		}
		return cached.vector;
	}

	private cacheEmbedding(text: string, vector: number[]): void {
		const key = Bun.hash
			.xxHash64(this.normalizeEmbeddingText(text))
			.toString(16);
		this.embeddingCache.set(key, {
			vector,
			expiresAt: Math.floor(Date.now() / 1000) + settings.embedding.cacheTtlSec,
		});
	}

	private async requestEmbeddingBatch(texts: string[]): Promise<number[][]> {
		const now = Date.now();
		if (now < this.embeddingUnavailableUntilMs) {
			throw new Error("embedding service temporarily unavailable");
		}

		const response = await fetch(`${this.embeddingBaseUrl}/embeddings`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({
				model: this.embeddingModel,
				input: texts,
				dimensions: this.embeddingDimension,
			}),
		});

		if (!response.ok) {
			this.embeddingUnavailableUntilMs = now + 5_000;
			if (!this.embeddingDegradedLogged) {
				logger.warn("Embedding service unavailable, using fallback", {
					status: response.status,
				});
				this.embeddingDegradedLogged = true;
			}
			throw new Error(`embedding service returned ${response.status}`);
		}

		const data = (await response.json()) as {
			data?: Array<{ embedding?: number[]; index?: number }>;
			embeddings?: number[][];
		};
		this.embeddingUnavailableUntilMs = 0;
		this.embeddingDegradedLogged = false;

		const vectors = new Array<number[]>(texts.length);
		if (data.data && data.data.length > 0) {
			for (const row of data.data) {
				if (
					typeof row.index === "number" &&
					row.index >= 0 &&
					row.index < texts.length &&
					row.embedding
				) {
					vectors[row.index] = this.normalizeEmbeddingDimensions(row.embedding);
				}
			}
		} else if (data.embeddings && data.embeddings.length > 0) {
			for (let i = 0; i < Math.min(data.embeddings.length, texts.length); i++) {
				vectors[i] = this.normalizeEmbeddingDimensions(data.embeddings[i]);
			}
		}

		for (let i = 0; i < vectors.length; i++) {
			if (!vectors[i]) {
				vectors[i] = this.generateFallbackVector(texts[i]);
			}
		}
		return vectors;
	}

	private async getEmbeddings(texts: string[]): Promise<number[][]> {
		if (texts.length === 0) {
			return [];
		}
		const vectors: number[][] = new Array(texts.length);
		const unresolved: Array<{ text: string; indices: number[] }> = [];
		const byNormalized = new Map<string, { text: string; indices: number[] }>();

		texts.forEach((text, index) => {
			const cached = this.getCachedEmbedding(text);
			if (cached) {
				vectors[index] = cached;
				return;
			}
			const normalized = this.normalizeEmbeddingText(text);
			const existing = byNormalized.get(normalized);
			if (existing) {
				existing.indices.push(index);
				return;
			}
			byNormalized.set(normalized, { text, indices: [index] });
		});

		unresolved.push(...byNormalized.values());
		if (unresolved.length === 0) {
			return vectors;
		}

		const chunkSize = Math.max(1, settings.embedding.batchSize);
		const chunks: Array<Array<{ text: string; indices: number[] }>> = [];
		for (let i = 0; i < unresolved.length; i += chunkSize) {
			chunks.push(unresolved.slice(i, i + chunkSize));
		}

		await this.mapWithConcurrency(
			chunks,
			settings.embedding.maxConcurrency,
			async (chunk) => {
				const batchTexts = chunk.map((item) => item.text);
				let batchVectors: number[][] | null = null;

				for (
					let attempt = 0;
					attempt <= settings.embedding.maxRetries;
					attempt++
				) {
					try {
						batchVectors = await this.requestEmbeddingBatch(batchTexts);
						break;
					} catch (error) {
						if (attempt === settings.embedding.maxRetries) {
							logger.warn("Embedding batch failed, using fallback vectors", {
								error: String(error),
								batchSize: batchTexts.length,
							});
						}
					}
				}

				if (!batchVectors) {
					batchVectors = batchTexts.map((text) =>
						this.generateFallbackVector(text),
					);
				}

				chunk.forEach((item, idx) => {
					const vector =
						batchVectors?.[idx] || this.generateFallbackVector(item.text);
					this.cacheEmbedding(item.text, vector);
					for (const originalIndex of item.indices) {
						vectors[originalIndex] = vector;
					}
				});
			},
		);

		return vectors;
	}

	private getDailyCollectionName(prefix: string, ts: number): string {
		const date = new Date(ts * 1000);
		const year = date.getFullYear();
		const month = String(date.getMonth() + 1).padStart(2, "0");
		const day = String(date.getDate()).padStart(2, "0");
		return `${prefix}_${year}_${month}_${day}`;
	}

	private getCollectionsForWindow(
		prefix: string,
		startTs: number,
		endTs: number,
	): string[] {
		const startDate = new Date(startTs * 1000);
		const endDate = new Date(endTs * 1000);
		const collections: string[] = [];

		const currentDate = new Date(startDate);
		while (currentDate <= endDate) {
			const year = currentDate.getFullYear();
			const month = String(currentDate.getMonth() + 1).padStart(2, "0");
			const day = String(currentDate.getDate()).padStart(2, "0");
			collections.push(`${prefix}_${year}_${month}_${day}`);
			currentDate.setDate(currentDate.getDate() + 1);
		}

		return collections;
	}

	private isNotFoundError(error: unknown): boolean {
		const message = String(error ?? "").toLowerCase();
		return message.includes("404") || message.includes("not found");
	}

	private async ensurePayloadIndex(
		collection: string,
		field_name: string,
		field_schema: PayloadIndexFieldSchema,
	): Promise<void> {
		try {
			await this.client.createPayloadIndex(collection, {
				field_name,
				field_schema,
			});
		} catch (error) {
			// Index creation is idempotent from our perspective; skip duplicates.
			logger.warn("Skipping payload index creation", {
				collection,
				field_name,
				error: String(error),
			});
		}
	}

	async setupCollections(): Promise<void> {
		logger.info("Ensuring Qdrant collections");

		try {
			const todayTs = Math.floor(Date.now() / 1000);
			const todayCollection = this.getDailyCollectionName(
				this.tier2CollectionPrefix,
				todayTs,
			);
			await this.ensureDailyTier2Collection(todayCollection);
		} catch (error) {
			logger.error("Error setting up collections", error);
			throw error;
		}
	}

	private async ensureDailyTier2Collection(
		collectionName: string,
	): Promise<void> {
		try {
			await this.client.getCollection(collectionName);
			return;
		} catch (error) {
			if (!this.isNotFoundError(error)) {
				logger.error("Error checking collection existence", error);
				throw error;
			}
		}

		try {
			logger.info("Creating daily Tier-2 collection", {
				collection: collectionName,
			});

			await this.client.createCollection(collectionName, {
				vectors: {
					log_dense_vector: {
						size: this.tier2Dim,
						distance: "Cosine",
						on_disk: true,
					},
				},
				sparse_vectors: {
					bm25_vector: {
						modifier: "idf",
					},
				},
				replication_factor: 1,
				shard_number: 1,
				quantization_config: {
					scalar: {
						type: "int8",
						quantile: 0.99,
						always_ram: true,
					},
				},
				optimizers_config: {
					default_segment_number: 2,
					indexing_threshold: 20000,
				},
			});

			await this.ensurePayloadIndex(collectionName, "start_ts", "integer");
			await this.ensurePayloadIndex(collectionName, "timestamp", "integer");
				await this.ensurePayloadIndex(collectionName, "event_id", "keyword");
				await this.ensurePayloadIndex(collectionName, "entity_id", "keyword");
				await this.ensurePayloadIndex(collectionName, "entity_hash", "keyword");
				await this.ensurePayloadIndex(collectionName, "group_key", "keyword");
				await this.ensurePayloadIndex(collectionName, "service", "keyword");
				await this.ensurePayloadIndex(collectionName, "rhythm_hash", "keyword");
			await this.ensurePayloadIndex(collectionName, "body", {
				type: "text",
				tokenizer: "word",
				lowercase: true,
			});
		} catch (error) {
			logger.error(`Error creating collection ${collectionName}`, error);
			throw error;
		}
	}

	async ingestToTier2(events: Tier2Event[]): Promise<void> {
		if (events.length === 0) return;

		// Group events by target collection
		const eventsByCollection = new Map<string, Tier2Event[]>();

		for (const event of events) {
			const startTsRaw = (event.payload.start_ts ??
				event.payload.timestamp ??
				Math.floor(Date.now() / 1000)) as number;
			const startTs = Number.isFinite(startTsRaw)
				? Math.floor(startTsRaw)
				: Math.floor(Date.now() / 1000);
			const collectionName = this.getDailyCollectionName(
				this.tier2CollectionPrefix,
				startTs,
			);

			if (!eventsByCollection.has(collectionName)) {
				eventsByCollection.set(collectionName, []);
			}
			eventsByCollection.get(collectionName)?.push(event);
		}

		const collectionEntries = Array.from(eventsByCollection.entries());
		await this.mapWithConcurrency(
			collectionEntries,
			settings.qdrant.maxConcurrentUpserts,
			async ([collection, collectionEvents]) => {
				await this.ensureDailyTier2Collection(collection);
				const denseVectors = new Array<number[]>(collectionEvents.length);
				const embedIndices: number[] = [];
				const embedTexts: string[] = [];
				for (let i = 0; i < collectionEvents.length; i++) {
					const payload = collectionEvents[i].payload;
					const confidence = Number(payload.confidence ?? 0);
					const severity = Number(payload.severity ?? 0);
					if (confidence >= 0.9 || severity >= 0.85) {
						embedIndices.push(i);
						embedTexts.push(collectionEvents[i].textForEmbedding);
					} else {
						denseVectors[i] = this.generateFallbackVector(
							collectionEvents[i].textForEmbedding,
						);
					}
				}
				if (embedTexts.length > 0) {
					const embedded = await this.getEmbeddings(embedTexts);
					for (let i = 0; i < embedIndices.length; i++) {
						const idx = embedIndices[i];
						denseVectors[idx] =
							embedded[i] ??
							this.generateFallbackVector(collectionEvents[idx].textForEmbedding);
					}
				}
				const points = collectionEvents.map((event, idx) => {
					const sparseVector = this.generateSparseVector(
						event.textForEmbedding,
					);
					return {
						id: this.generateId(),
						vector: {
							log_dense_vector:
								denseVectors[idx] ??
								this.generateFallbackVector(event.textForEmbedding),
							bm25_vector: sparseVector,
						},
						payload: event.payload,
					};
				});
				await this.client.upsert(collection, {
					points,
					wait: false,
				});
			},
		);
	}

	private async refineWithContextFeedback(
		collection: string,
		filter: Record<string, unknown>,
		hits: QdrantScoredPoint[],
		limit: number,
	): Promise<QdrantScoredPoint[]> {
		if (hits.length < 4) {
			return hits.slice(0, limit);
		}
		const positives = hits
			.filter((hit) => {
				const payload = (hit.payload || {}) as Record<string, unknown>;
				return (
					Number(payload.confidence ?? 0) >= 0.85 ||
					Number(payload.severity ?? 0) >= 0.8 ||
					Number(payload.score ?? hit.score ?? 0) >= 0.9
				);
			})
			.map((hit) => hit.id);
		const negatives = hits
			.filter((hit) => {
				const payload = (hit.payload || {}) as Record<string, unknown>;
				return (
					Number(payload.confidence ?? 0) <= 0.4 &&
					Number(payload.severity ?? 0) <= 0.5 &&
					Number(payload.score ?? hit.score ?? 0) <= 0.7
				);
			})
			.map((hit) => hit.id);
		const pairCount = Math.min(positives.length, negatives.length, 8);
		if (pairCount < 2) {
			return hits.slice(0, limit);
		}
		const context = Array.from({ length: pairCount }).map((_, i) => ({
			positive: positives[i],
			negative: negatives[i],
		}));

		try {
			const queryClient = this.client as unknown as {
				query: (collectionName: string, payload: Record<string, unknown>) => Promise<{ points?: QdrantScoredPoint[] } | QdrantScoredPoint[]>;
			};
			const response = await queryClient.query(collection, {
				query: { context },
				using: "log_dense_vector",
				filter,
				limit,
				with_payload: true,
			});
			const points = Array.isArray(response)
				? response
				: ((response.points ?? []) as QdrantScoredPoint[]);
			if (points.length > 0) {
				return points;
			}
		} catch {
			// Fallback to original hits if context query is not supported or fails.
		}
		return hits.slice(0, limit);
	}

	async findTier2Clusters(
		startTs: number,
		endTs: number,
		textFilter?: string,
	): Promise<QdrantScoredPoint[]> {
		const collections = this.getCollectionsForWindow(
			this.tier2CollectionPrefix,
			startTs,
			endTs,
		);

		if (collections.length === 0) {
			return [];
		}

		const queryFilter: Record<string, unknown> = {
			must: [
				{
					key: "start_ts",
					range: { gte: startTs, lte: endTs },
				},
			],
		};
		if (textFilter) {
			(queryFilter.must as Record<string, unknown>[]).push({
				key: "context",
				match: { text: textFilter },
			});
		}

		try {
			const groupedResults = await Promise.all(
				collections.map(async (collection) => {
					try {
						if (textFilter) {
							const vector = await this.getEmbedding(textFilter);
							return await this.client.queryGroups(collection, {
								query: vector,
								using: "log_dense_vector",
								filter: queryFilter,
								group_by: "group_key",
								group_size: 3,
								limit: 100,
								with_payload: true,
								score_threshold: 0.15,
							});
						}
						return await this.client.queryGroups(collection, {
							filter: queryFilter,
							group_by: "group_key",
							group_size: 3,
							limit: 100,
							with_payload: true,
						});
					} catch {
						return { groups: [] };
					}
				}),
			);

			const grouped = new Map<string, QdrantScoredPoint>();
			for (const result of groupedResults) {
				const groups = (result as { groups?: Array<{ id: string | number; hits?: Array<QdrantScoredPoint> }> }).groups ?? [];
				for (const group of groups) {
					const topHit = group.hits?.[0];
					if (!topHit) continue;
					const key = String(group.id);
					if (grouped.has(key)) continue;
					grouped.set(key, {
						...topHit,
						payload: {
							...(topHit.payload ?? {}),
							group_key: group.id,
							count: group.hits?.length ?? 1,
						},
					});
				}
			}

			if (grouped.size === 0) {
				// Fallback for clusters when grouping is unavailable.
				const results = await Promise.all(
					collections.map((collection) =>
						this.client
							.scroll(collection, {
								filter: queryFilter,
								limit: 200,
								with_payload: true,
							})
							.catch(() => ({ points: [] as QdrantScoredPoint[] })),
					),
				);
				for (const result of results) {
					const points =
						(result as { points?: QdrantScoredPoint[] }).points ?? [];
					for (const point of points) {
						const groupKey = String(
							point.payload?.group_key ??
							point.payload?.rhythm_hash ??
								point.payload?.entity_hash ??
								point.payload?.entity_id ??
								point.id,
						);
						if (!grouped.has(groupKey)) {
							grouped.set(groupKey, point);
						}
					}
				}
			}

			const baseHits = Array.from(grouped.values());
			const refinedByCollection = await Promise.all(
				collections.map(async (collection) => {
					const localHits = baseHits.filter((hit) => {
						const ts = Number(hit.payload?.start_ts ?? hit.payload?.timestamp ?? 0);
						const localCollection = this.getDailyCollectionName(
							this.tier2CollectionPrefix,
							Number.isFinite(ts) ? Math.floor(ts) : startTs,
						);
						return localCollection === collection;
					});
					return this.refineWithContextFeedback(
						collection,
						queryFilter,
						localHits,
						100,
					);
				}),
			);

			return refinedByCollection.flat().slice(0, 200);
		} catch (error) {
			logger.error("Error finding Tier 2 clusters", error);
			return [];
		}
	}

	async triageSimilarEvents(
		positiveIds: string[],
		negativeIds: string[],
		startTs: number,
		endTs: number,
	): Promise<QdrantScoredPoint[]> {
		const collections = this.getCollectionsForWindow(
			this.tier2CollectionPrefix,
			startTs,
			endTs,
		);

		if (collections.length === 0 || positiveIds.length === 0) {
			return [];
		}

		try {
			const filter = {
				must: [
					{
						key: "start_ts",
						range: { gte: startTs, lte: endTs },
					},
				],
			};
			const groupedResults = await Promise.all(
				collections.map(async (collection) => {
					try {
						return await this.client.recommendPointGroups(collection, {
							positive: positiveIds,
							negative: negativeIds,
							using: "log_dense_vector",
							filter,
							group_by: "group_key",
							group_size: 3,
							limit: 50,
							with_payload: true,
							score_threshold: 0.1,
						});
					} catch {
						const hits = await this.client
							.recommend(collection, {
								positive: positiveIds,
								negative: negativeIds,
								using: "log_dense_vector",
								filter,
								limit: 50,
								with_payload: true,
							})
							.catch(() => []);
						return { groups: [{ id: "fallback", hits }] };
					}
				}),
			);
			const allHits = groupedResults.flatMap((result) => {
				const groups = (result as { groups?: Array<{ hits?: QdrantScoredPoint[] }> }).groups ?? [];
				return groups.flatMap((group) => group.hits ?? []);
			});

			allHits.sort((a, b) => (b.score || 0) - (a.score || 0));

			return allHits.slice(0, 50) as QdrantScoredPoint[];
		} catch (error) {
			logger.error("Error triaging similar events", error);
			return [];
		}
	}

	private async getEmbedding(text: string): Promise<number[]> {
		const cached = this.getCachedEmbedding(text);
		if (cached) {
			return cached;
		}

		const now = Date.now();
		if (now < this.embeddingUnavailableUntilMs) {
			return this.generateFallbackVector(text);
		}

		try {
			const response = await fetch(`${this.embeddingBaseUrl}/embeddings`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({
					model: this.embeddingModel,
					input: text,
					dimensions: this.embeddingDimension,
				}),
			});

			if (!response.ok) {
				this.embeddingUnavailableUntilMs = now + 5_000;
				if (!this.embeddingDegradedLogged) {
					logger.warn("Embedding service unavailable, using fallback", {
						status: response.status,
					});
					this.embeddingDegradedLogged = true;
				}
				const fallback = this.generateFallbackVector(text);
				this.cacheEmbedding(text, fallback);
				return fallback;
			}

			const data = (await response.json()) as {
				data?: Array<{ embedding?: number[] }>;
				embeddings?: number[][];
			};
			this.embeddingUnavailableUntilMs = 0;
			this.embeddingDegradedLogged = false;

			const openAiVector = data.data?.[0]?.embedding;
			const legacyVector = data.embeddings?.[0];
			const selected = openAiVector || legacyVector;
			const vector = selected
				? this.normalizeEmbeddingDimensions(selected)
				: this.generateFallbackVector(text);
			this.cacheEmbedding(text, vector);
			return vector;
		} catch (error) {
			this.embeddingUnavailableUntilMs = now + 5_000;
			if (!this.embeddingDegradedLogged) {
				logger.warn("Embedding request failed, using fallback", {
					error: String(error),
				});
				this.embeddingDegradedLogged = true;
			}
			const fallback = this.generateFallbackVector(text);
			this.cacheEmbedding(text, fallback);
			return fallback;
		}
	}

	private normalizeEmbeddingDimensions(vector: number[]): number[] {
		if (vector.length === this.tier2Dim) {
			return vector;
		}
		if (vector.length > this.tier2Dim) {
			return vector.slice(0, this.tier2Dim);
		}
		const padded = vector.slice();
		while (padded.length < this.tier2Dim) {
			padded.push(0);
		}
		return padded;
	}

	private generateFallbackVector(text: string): number[] {
		const hash = Bun.hash.xxHash64(text);
		const vector: number[] = [];

		for (let i = 0; i < this.tier2Dim; i++) {
			const byteIndex = Math.floor(i / 8);
			const byteValue = Number((hash >> BigInt(byteIndex * 8)) & 0xffn);
			const bitIndex = i % 8;
			const bitValue = (byteValue >> (7 - bitIndex)) & 1;
			vector.push(bitValue === 1 ? 1.0 : 0.0);
		}

		return vector;
	}

	private generateSparseVector(text: string): {
		indices: number[];
		values: number[];
	} {
		const words = text.toLowerCase().split(/\s+/);
		const freq = new Map<string, number>();
		for (const word of words) {
			freq.set(word, (freq.get(word) ?? 0) + 1);
		}
		const byIndex = new Map<number, number>();
		for (const [word, count] of freq.entries()) {
			const hash = Bun.hash.xxHash64(word);
			const hashValue = Number(hash & 0xffffffffn);
			const index = hashValue % 16384;
			const value = 1 + count / Math.max(words.length, 1);
			byIndex.set(index, (byIndex.get(index) ?? 0) + value);
		}

		const sorted = Array.from(byIndex.entries()).sort((a, b) => a[0] - b[0]);
		const indices = sorted.map(([index]) => index);
		const values = sorted.map(([, value]) => value);

		return { indices, values };
	}

	private generateId(): string {
		return crypto.randomUUID();
		return crypto.randomUUID();
	}
}
