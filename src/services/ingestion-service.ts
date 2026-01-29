import type { QueueWorker } from "../queue/worker";
import type { BatchSummary, LogRecord } from "../types";
import { generateRhythmHash } from "./ingestion-helpers";
import type { QdrantService } from "./qdrant-service";
import type { Tier1Engine } from "./tier1-engine";

export class IngestionService {
	private qdrantService: QdrantService;
	private worker: QueueWorker;

	constructor(
		qdrantService: QdrantService,
		_tier1Engine: Tier1Engine,
		worker: QueueWorker,
	) {
		this.qdrantService = qdrantService;
		this.worker = worker;
	}

	async ingestLogBatch(logs: unknown[]): Promise<BatchSummary> {
		const pointsToPrepare: Array<{
			id: string;
			vector: number[];
			payload: Record<string, unknown>;
		}> = [];
		const groups = new Map<
			string,
			{ service: string; count: number; uniqueIds: string[] }
		>();

		for (const raw of logs) {
			const log = this.parseLog(raw);

			pointsToPrepare.push({
				id: log.id,
				vector: this.getSemanticVector(log.rhythmHash),
				payload: {
					rhythm_hash: log.rhythmHash,
					service: log.service,
					severity: log.severity,
					ts: log.timestamp,
					body: log.body,
					full_log_json: raw,
				},
			});

			const groupKey = log.rhythmHash;
			if (!groups.has(groupKey)) {
				groups.set(groupKey, {
					service: log.service,
					count: 0,
					uniqueIds: [],
				});
			}

			const group = groups.get(groupKey);
			if (group) {
				group.count++;
				group.uniqueIds.push(log.id);
			}

			await this.worker.enqueue(log);
		}

		const summary: BatchSummary = {
			timestamp: Date.now() / 1000,
			totalLogs: pointsToPrepare.length,
			groups: Array.from(groups.values()).map((group) => ({
				rhythmHash: group.service,
				service: group.service,
				count: group.count,
				uniqueIds: group.uniqueIds,
			})),
		};

		await this.qdrantService.upsertTier1Points(
			pointsToPrepare.map((pt) => ({
				id: pt.id,
				vector: pt.vector,
				payload: pt.payload,
			})),
		);

		return summary;
	}

	private parseLog(raw: unknown): LogRecord {
		const rawObj = raw as {
			resourceLogs?: Array<{
				scopeLogs?: Array<{
					logRecords?: Array<{
						severityText?: string;
						timeUnixNano?: string;
						body?: { stringValue?: string };
					}>;
				}>;
				resource?: {
					attributes?: Array<{
						key: string;
						value?: Record<string, unknown>;
					}>;
				};
			}>;
		};

		const rl = rawObj.resourceLogs?.[0] || {};
		const scope = rl.scopeLogs?.[0] || {};
		const rec = scope.logRecords?.[0] || {};

		const rattrs = (rl.resource?.attributes || []).reduce(
			(acc: Record<string, unknown>, attr) => {
				if (attr.key) {
					const value = attr.value;
					if (value && typeof value === "object") {
						const vals = Object.values(value);
						acc[attr.key] = vals[0];
					}
				}
				return acc;
			},
			{},
		);

		const service = String(rattrs["service.name"] || "unknown");
		const severity = String(rec.severityText || "INFO");
		const tsS = String(rec.timeUnixNano || "0");
		const ts = Math.floor(Number(tsS) / 1_000_000_000);
		const body = String(rec.body?.stringValue || "");

		return {
			id: this.generateId(),
			timestamp: ts,
			service,
			severity,
			body,
			rhythmHash: generateRhythmHash({
				resource: { attributes: { "service.name": service } },
				severityText: severity,
				body: { stringValue: body },
			}),
			attributes: rattrs,
			fullLogJson: raw,
		};
	}

	private generateId(): string {
		const hash = Bun.hash.xxHash64(`${Date.now()}-${Math.random()}`);
		return hash.toString(16);
	}

	private getSemanticVector(rhythmHash: string): number[] {
		const hash = Bun.hash.xxHash64(rhythmHash);
		const vector: number[] = [];

		for (let i = 0; i < 64; i++) {
			const byteIndex = Math.floor(i / 8);
			const byteValue = Number((hash >> BigInt(byteIndex * 8)) & 0xffn);
			const bitIndex = i % 8;
			const bitValue = (byteValue >> (7 - bitIndex)) & 1;
			vector.push(bitValue === 1 ? 1.0 : 0.0);
		}

		return vector;
	}
}
