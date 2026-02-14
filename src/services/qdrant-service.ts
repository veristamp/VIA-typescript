import { QdrantClient } from "@qdrant/js-client-rest";
import { settings } from "../config/settings";
import { logger } from "../utils/logger";

export interface QdrantPoint {
	id: string;
	vector: number[];
	payload: Record<string, unknown>;
}

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
	private tier1Dim: number = 64;
	private tier2Dim: number = 384;
	private tier1CollectionPrefix: string = "via_tier1_monitor";
	private tier2CollectionPrefix: string = "via_forensic_index_v2";
	private embeddingUrl: string;
	private embeddingCache = new Map<
		string,
		{ vector: number[]; expiresAt: number }
	>();

	constructor() {
		this.client = new QdrantClient({
			url: `http://${settings.qdrant.host}:${settings.qdrant.port}`,
		});
		this.embeddingUrl = process.env.EMBEDDING_URL || "http://localhost:8080";
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
		const response = await fetch(`${this.embeddingUrl}/embeddings`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ input: texts }),
		});

		if (!response.ok) {
			throw new Error(`embedding service returned ${response.status}`);
		}

		const data = (await response.json()) as { embeddings?: number[][] };
		if (!Array.isArray(data.embeddings) || data.embeddings.length !== texts.length) {
			throw new Error("embedding service returned invalid batch shape");
		}
		return data.embeddings;
	}

	private async getEmbeddings(texts: string[]): Promise<number[][]> {
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

				for (let attempt = 0; attempt <= settings.embedding.maxRetries; attempt++) {
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
					batchVectors = batchTexts.map((text) => this.generateFallbackVector(text));
				}

				chunk.forEach((item, idx) => {
					const vector = batchVectors?.[idx] || this.generateFallbackVector(item.text);
					this.cacheEmbedding(item.text, vector);
					for (const originalIndex of item.indices) {
						vectors[originalIndex] = vector;
					}
				});
			},
		);

		return vectors;
	}

	get tier1Dimension(): number {
		return this.tier1Dim;
	}

	get tier2Dimension(): number {
		return this.tier2Dim;
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

	private async ensureTier1Collection(): Promise<void> {
		try {
			await this.client.getCollection(this.tier1CollectionPrefix);
			return;
		} catch (error) {
			if (!this.isNotFoundError(error)) {
				throw error;
			}
		}

		logger.info("Creating Tier-1 collection", {
			collection: this.tier1CollectionPrefix,
		});
		await this.client.createCollection(this.tier1CollectionPrefix, {
			vectors: {
				size: this.tier1Dim,
				distance: "Dot",
			},
			quantization_config: {
				binary: {
					always_ram: true,
				},
			},
			replication_factor: 1,
			shard_number: 1,
		});
	}

	async setupCollections(): Promise<void> {
		logger.info("Ensuring Qdrant collections");

		try {
			await this.ensureTier1Collection();
			await this.ensurePayloadIndex(
				this.tier1CollectionPrefix,
				"ts",
				"integer",
			);

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

	async upsertTier1Points(points: QdrantPoint[]): Promise<number> {
		if (!points || points.length === 0) {
			return 0;
		}

		const qdrantPoints = points.map((pt) => ({
			id: pt.id,
			vector: pt.vector,
			payload: pt.payload,
		}));

		await this.client.upsert(this.tier1CollectionPrefix, {
			points: qdrantPoints,
			wait: false,
		});

		return points.length;
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
				const denseVectors = await this.getEmbeddings(
					collectionEvents.map((event) => event.textForEmbedding),
				);
				const points = collectionEvents.map((event, idx) => {
					const sparseVector = this.generateSparseVector(event.textForEmbedding);
					return {
						id: this.generateId(),
						vector: {
							log_dense_vector: denseVectors[idx],
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

	async getPointsFromTier1(startTs: number, endTs: number): Promise<unknown[]> {
		try {
			const result = await this.client.scroll(this.tier1CollectionPrefix, {
				limit: 100000,
				with_payload: true,
				with_vector: true,
				filter: {
					must: [
						{
							key: "ts",
							range: {
								gte: startTs,
								lte: endTs,
							},
						},
					],
				},
			});

			const points = (result as { points?: unknown[] })?.points || [];

			return points;
		} catch (error) {
			logger.error("Error getting points from Tier 1", error);
			return [];
		}
	}

	async getHistoricalBaseline(
		windowStartTs: number,
		sampleSize: number = 10000,
	): Promise<unknown[]> {
		try {
			const result = await this.client.scroll(this.tier1CollectionPrefix, {
				limit: sampleSize,
				with_payload: true,
				with_vector: true,
				order_by: {
					key: "ts",
					direction: "desc",
				},
				filter: {
					must: [
						{
							key: "ts",
							range: {
								lt: windowStartTs,
							},
						},
					],
				},
			});

			const points = (result as { points?: unknown[] })?.points || [];

			return points;
		} catch (error) {
			logger.error("Error getting historical baseline", error);
			return [];
		}
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

		let queryVector: number[] | null = null;
		let queryFilter: Record<string, unknown> | null = null;

		if (textFilter) {
			queryVector = await this.getEmbedding(textFilter);
			queryFilter = {
				must: [
					{
						key: "body",
						match: {
							text: textFilter,
						},
					},
				],
			};
		} else {
			queryVector = new Array(this.tier2Dim).fill(0);
		}

		const namedVector = {
			name: "log_dense_vector",
			vector: queryVector,
		};

		const searchPromises = collections.map((collection) =>
			this.client.search(collection, {
				vector: namedVector,
				filter: queryFilter,
				limit: 100,
				with_payload: true,
			}),
		);

		try {
			const results = await Promise.all(
				searchPromises.map((promise) => promise.catch(() => [])),
			);
			const allHits = results.flatMap((r) => r || []);

			const grouped = new Map<string, QdrantScoredPoint>();
			for (const hit of allHits) {
				const rhythmHash = hit.payload?.rhythm_hash as string | undefined;
				if (rhythmHash && !grouped.has(rhythmHash)) {
					grouped.set(rhythmHash, hit as QdrantScoredPoint);
				}
			}

			return Array.from(grouped.values());
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

		const recommendPromises = collections.map((collection) =>
			this.client.recommend(collection, {
				positive: positiveIds,
				negative: negativeIds,
				using: "log_dense_vector",
				limit: 50,
				with_payload: true,
			}),
		);

		try {
			const results = await Promise.all(
				recommendPromises.map((promise) => promise.catch(() => [])),
			);
			const allHits = results.flatMap((r) => r || []);

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
		try {
			const response = await fetch(`${this.embeddingUrl}/embeddings`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ input: text }),
			});

			if (!response.ok) {
				logger.warn("Embedding service unavailable, using fallback", {
					status: response.status,
				});
				const fallback = this.generateFallbackVector(text);
				this.cacheEmbedding(text, fallback);
				return fallback;
			}

			const data = (await response.json()) as { embeddings?: number[][] };
			const vector = data.embeddings?.[0] || this.generateFallbackVector(text);
			this.cacheEmbedding(text, vector);
			return vector;
		} catch (error) {
			logger.warn("Embedding request failed, using fallback", {
				error: String(error),
			});
			const fallback = this.generateFallbackVector(text);
			this.cacheEmbedding(text, fallback);
			return fallback;
		}
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

	private generateSparseVector(text: string): { indices: number[]; values: number[] } {
		const words = text.toLowerCase().split(/\s+/);
		const freq = new Map<string, number>();
		for (const word of words) {
			freq.set(word, (freq.get(word) ?? 0) + 1);
		}
		const indices: number[] = [];
		const values: number[] = [];

		for (const [word, count] of freq.entries()) {
			const hash = Bun.hash.xxHash64(word);
			const hashValue = Number(hash & 0xffffffffn);
			indices.push(hashValue % 16384);
			values.push(1 + count / Math.max(words.length, 1));
		}

		return { indices, values };
	}

	private generateId(): string {
		const hash = Bun.hash.xxHash64(`${Date.now()}-${Math.random()}`);
		return hash.toString(16);
	}
}
