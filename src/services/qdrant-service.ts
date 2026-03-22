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
}

export class QdrantService {
	private client: QdrantClient;
	private tier2Dim: number;
	private tier2CollectionPrefix = "via_forensic_index_v3"; // Bump version for new vector strategy
	private embeddingBaseUrl: string;
	private useExternalEmbedding: boolean;

	constructor() {
		this.client = new QdrantClient({
			url: `http://${settings.qdrant.host}:${settings.qdrant.port}`,
		});
		
		this.embeddingBaseUrl = process.env.EMBEDDING_BASE_URL || "";
		// Only use external if URL is explicitly provided
		this.useExternalEmbedding = this.embeddingBaseUrl.length > 0;
		this.tier2Dim = Number.parseInt(process.env.EMBEDDING_DIMENSION || "64", 10);
	}

	/**
	 * ULTRA-FAST STRUCTURAL EMBEDDING (0MB Model)
	 * Captures log structure and token similarity using the Hashing Trick.
	 * Microsecond latency on CPU.
	 */
	private generateStructuralEmbedding(text: string): number[] {
		const tokens = text.toLowerCase().split(/[^a-z0-9]+/);
		const vector = new Float32Array(this.tier2Dim).fill(0);
		
		for (const token of tokens) {
			if (token.length < 2) continue;
			
			// Double hashing for sign and index (avoids collision bias)
			const h1 = Bun.hash.xxHash32(token, 0);
			const h2 = Bun.hash.xxHash32(token, 1);
			
			const index = Math.abs(h1) % this.tier2Dim;
			const sign = (h2 % 2 === 0) ? 1 : -1;
			
			vector[index] += sign;
		}
		
		// L2 Normalization
		let norm = 0;
		for (let i = 0; i < vector.length; i++) norm += vector[i] * vector[i];
		norm = Math.sqrt(norm);
		
		if (norm > 0) {
			for (let i = 0; i < vector.length; i++) vector[i] /= norm;
		}
		
		return Array.from(vector);
	}

	private async getEmbeddings(texts: string[]): Promise<number[][]> {
		if (!this.useExternalEmbedding) {
			return texts.map(t => this.generateStructuralEmbedding(t));
		}

		try {
			const response = await fetch(`${this.embeddingBaseUrl}/embeddings`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ input: texts }),
			});

			if (response.ok) {
				const data = await response.json() as any;
				return data.data.map((d: any) => d.embedding);
			}
		} catch (e) {
			logger.warn("External embedding failed, falling back to structural", { error: String(e) });
		}

		return texts.map(t => this.generateStructuralEmbedding(t));
	}

	private getDailyCollectionName(ts: number): string {
		const date = new Date(ts * 1000);
		return `${this.tier2CollectionPrefix}_${date.toISOString().split("T")[0].replace(/-/g, "_")}`;
	}

	private getCollectionsForWindow(startTs: number, endTs: number): string[] {
		const collections: string[] = [];
		let current = startTs;
		while (current <= endTs + 86400) {
			collections.push(this.getDailyCollectionName(current));
			current += 86400;
		}
		return Array.from(new Set(collections));
	}

	async setupCollections(name?: string): Promise<void> {
		const target = name || this.getDailyCollectionName(Date.now() / 1000);
		try {
			await this.client.getCollection(target);
		} catch {
			logger.info("Creating optimized daily collection", { collection: target });
			await this.client.createCollection(target, {
				vectors: { log_dense_vector: { size: this.tier2Dim, distance: "Cosine" } }
			});
			await this.client.createPayloadIndex(target, { field_name: "group_key", field_schema: "keyword" });
			await this.client.createPayloadIndex(target, { field_name: "start_ts", field_schema: "integer" });
		}
	}

	async ingestToTier2(events: Tier2Event[]): Promise<void> {
		if (events.length === 0) return;
		const name = this.getDailyCollectionName(Date.now() / 1000);
		await this.setupCollections(name);

		const embeddings = await this.getEmbeddings(events.map(e => e.textForEmbedding));
		const points = events.map((e, i) => ({
			id: crypto.randomUUID(),
			vector: { log_dense_vector: embeddings[i] },
			payload: e.payload,
		}));

		await this.client.upsert(name, { points, wait: false });
	}

	async findTier2Clusters(startTs: number, endTs: number, textFilter?: string): Promise<QdrantScoredPoint[]> {
		const collections = this.getCollectionsForWindow(startTs, endTs);
		const filter: any = { must: [{ key: "start_ts", range: { gte: startTs, lte: endTs } }] };
		if (textFilter) filter.must.push({ key: "context", match: { text: textFilter } });

		const allHits: QdrantScoredPoint[] = [];
		for (const name of collections) {
			try {
				const result = await this.client.queryGroups(name, {
					filter,
					group_by: "group_key",
					group_size: 10,
					limit: 50,
					with_payload: true,
				});
				allHits.push(...result.groups.flatMap(g => g.hits.map(h => ({
					...h,
					payload: { ...(h.payload || {}), group_key: g.id }
				}))));
			} catch {}
		}
		return allHits;
	}

	async triageSimilarEvents(queryText: string, startTs: number, endTs: number): Promise<QdrantScoredPoint[]> {
		const collections = this.getCollectionsForWindow(startTs, endTs);
		const [vector] = await this.getEmbeddings([queryText]);
		
		const allHits: QdrantScoredPoint[] = [];
		for (const name of collections) {
			try {
				const result = await this.client.query(name, {
					query: vector,
					filter: { must: [{ key: "start_ts", range: { gte: startTs, lte: endTs } }] },
					limit: 20,
					with_payload: true
				});
				allHits.push(...(result.points as any));
			} catch {}
		}
		return allHits.sort((a, b) => b.score - a.score).slice(0, 50);
	}
}
