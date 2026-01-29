export const settings = {
	tier1: {
		ewmaHalfLife: 60,
		hllPrecision: 14,
		cusum: {
			target: 0,
			slack: 2.0,
			threshold: 5.0,
		},
	},
	queue: {
		maxSize: 10000,
		batchSize: 100,
		flushInterval: 1000,
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
	},
} as const;
