export const settings = {
	queue: {
		maxSize: 50000,
		batchSize: 500,
		flushInterval: 500,
		maxWorkers: 16,
		retryBaseDelayMs: 200,
	},
	embedding: {
		batchSize: 64,
		maxConcurrency: 4,
		cacheTtlSec: 900,
		maxRetries: 2,
	},
	server: {
		port: 3000,
		host: "0.0.0.0",
	},
	postgres: {
		host: "localhost",
		port: 5432,
		database: "via_registry",
		user: "via",
		password: "via",
	},
	qdrant: {
		host: "localhost",
		port: 6333,
		maxConcurrentUpserts: 1,
	},
	tier1: {
		baseUrl:
			process.env.TIER1_BASE_URL?.trim() || "http://127.0.0.1:3001",
	},
} as const;
