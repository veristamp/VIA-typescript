export const settings = {
	queue: {
		maxSize: 100000,
		batchSize: 1000,
		flushInterval: 250,
		maxWorkers: 32,
		retryBaseDelayMs: 100,
	},
	embedding: {
		batchSize: 128,
		maxConcurrency: 8,
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
