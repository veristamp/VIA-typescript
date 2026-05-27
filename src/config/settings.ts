function readString(name: string, fallback: string): string {
	return process.env[name] || fallback;
}

function readNumber(name: string, fallback: number): number {
	const value = process.env[name];
	if (!value) {
		return fallback;
	}
	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : fallback;
}

function readEmbeddingMode(): "hash" | "external" {
	return process.env.EMBEDDING_MODE === "external" ? "external" : "hash";
}

export const settings = {
	queue: {
		maxSize: readNumber("TIER2_QUEUE_MAX_SIZE", 10000),
		batchSize: readNumber("TIER2_QUEUE_BATCH_SIZE", 100),
		flushInterval: readNumber("TIER2_QUEUE_FLUSH_INTERVAL_MS", 1000),
		maxWorkers: readNumber("TIER2_QUEUE_MAX_WORKERS", 4),
		retryBaseDelayMs: readNumber("TIER2_QUEUE_RETRY_BASE_DELAY_MS", 200),
	},
	embedding: {
		mode: readEmbeddingMode(),
		baseUrl: readString("EMBEDDING_BASE_URL", "http://127.0.0.1:1234/v1"),
		model: readString(
			"EMBEDDING_MODEL",
			"text-embedding-nomic-embed-text-v1.5",
		),
		dimension: readNumber("EMBEDDING_DIMENSION", 64),
		batchSize: readNumber("EMBEDDING_BATCH_SIZE", 32),
		maxConcurrency: readNumber("EMBEDDING_MAX_CONCURRENCY", 1),
		cacheTtlSec: readNumber("EMBEDDING_CACHE_TTL_SEC", 900),
		maxRetries: readNumber("EMBEDDING_MAX_RETRIES", 2),
	},
	server: {
		port: readNumber("TIER2_HTTP_PORT", 3000),
		host: readString("TIER2_HTTP_HOST", "0.0.0.0"),
	},
	rpc: {
		port: readNumber("TIER2_GRPC_PORT", 3002),
		host: readString("TIER2_GRPC_HOST", "0.0.0.0"),
	},
	postgres: {
		host: readString("POSTGRES_HOST", "localhost"),
		port: readNumber("POSTGRES_PORT", 5432),
		database: readString("POSTGRES_DB", "via_registry"),
		user: readString("POSTGRES_USER", "via"),
		password: readString("POSTGRES_PASSWORD", "via"),
	},
	qdrant: {
		host: readString("QDRANT_HOST", "localhost"),
		port: readNumber("QDRANT_PORT", 6333),
		maxConcurrentUpserts: readNumber("QDRANT_MAX_CONCURRENT_UPSERTS", 1),
	},
} as const;
