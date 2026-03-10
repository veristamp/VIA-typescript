import { logger } from "../utils/logger";

export interface LGTMConfig {
	lokiUrl: string;
	tempoUrl: string;
	prometheusUrl: string;
}

export class LGTMService {
	private config: LGTMConfig;

	constructor(config?: Partial<LGTMConfig>) {
		this.config = {
			lokiUrl: config?.lokiUrl || process.env.LOKI_URL || "http://mono-auth-loki:3100",
			tempoUrl: config?.tempoUrl || process.env.TEMPO_URL || "http://mono-auth-tempo:3200",
			prometheusUrl: config?.prometheusUrl || process.env.PROMETHEUS_URL || "http://mono-auth-prometheus:9090",
		};
	}

	async getTrace(traceId: string) {
		try {
			const response = await fetch(`${this.config.tempoUrl}/api/traces/${traceId}`);
			if (!response.ok) {
				logger.warn("Failed to fetch trace from Tempo", { traceId, status: response.status });
				return null;
			}
			return await response.json();
		} catch (error) {
			logger.error("Error fetching trace from Tempo", { traceId, error });
			return null;
		}
	}

	async getLogs(query: string, startTs: number, endTs: number, limit = 100) {
		try {
			const params = new URLSearchParams({
				query,
				start: (startTs * 1000000000).toString(), // Nanoseconds
				end: (endTs * 1000000000).toString(),
				limit: limit.toString(),
			});
			const response = await fetch(`${this.config.lokiUrl}/loki/api/v1/query_range?${params.toString()}`);
			if (!response.ok) {
				logger.warn("Failed to fetch logs from Loki", { query, status: response.status });
				return null;
			}
			return await response.json();
		} catch (error) {
			logger.error("Error fetching logs from Loki", { query, error });
			return null;
		}
	}

	async getMetrics(query: string, startTs: number, endTs: number, step = "15s") {
		try {
			const params = new URLSearchParams({
				query,
				start: startTs.toString(),
				end: endTs.toString(),
				step,
			});
			const response = await fetch(`${this.config.prometheusUrl}/api/v1/query_range?${params.toString()}`);
			if (!response.ok) {
				logger.warn("Failed to fetch metrics from Prometheus", { query, status: response.status });
				return null;
			}
			return await response.json();
		} catch (error) {
			logger.error("Error fetching metrics from Prometheus", { query, error });
			return null;
		}
	}
}
