import { logger } from "../utils/logger";
import type { ForensicAnalysisService } from "./forensic-analysis-service";
import type { QdrantService } from "./qdrant-service";

export interface LegacyAnomalySignal {
	t: number;
	u: string;
	score: number;
	severity: number;
	type: number;
}

export interface Tier1AnomalySignalV1 {
	schema_version?: number;
	entity_hash: number;
	timestamp: number;
	score: number;
	severity: number;
	primary_detector: number;
	detectors_fired: number;
	confidence: number;
	detector_scores: number[];
}

export type IncomingAnomalySignal = LegacyAnomalySignal | Tier1AnomalySignalV1;

interface NormalizedAnomalySignal {
	schemaVersion: number;
	entityHash: number | null;
	entityId: string;
	timestamp: number;
	score: number;
	severity: number;
	signalType: number;
	detectorsFired: number;
	confidence: number;
	detectorScores: number[];
}

export class Tier2Service {
	constructor(
		private qdrant: QdrantService,
		private forensic: ForensicAnalysisService,
	) {}

	private normalizeToUnixSeconds(ts: number): number {
		if (!Number.isFinite(ts) || ts <= 0) {
			return Math.floor(Date.now() / 1000);
		}
		// ns
		if (ts > 1e15) return Math.floor(ts / 1e9);
		// ms
		if (ts > 1e12) return Math.floor(ts / 1e3);
		return Math.floor(ts);
	}

	private normalizeSignal(
		signal: IncomingAnomalySignal,
	): NormalizedAnomalySignal {
		if ("entity_hash" in signal && "timestamp" in signal) {
			const entityHash = signal.entity_hash;
			const timestamp = this.normalizeToUnixSeconds(signal.timestamp);
			return {
				schemaVersion: signal.schema_version ?? 1,
				entityHash,
				entityId: `hash:${entityHash}`,
				timestamp,
				score: signal.score,
				severity: signal.severity,
				signalType: signal.primary_detector,
				detectorsFired: signal.detectors_fired,
				confidence: signal.confidence,
				detectorScores: signal.detector_scores ?? [],
			};
		}

		const timestamp = this.normalizeToUnixSeconds(signal.t);
		return {
			schemaVersion: 0,
			entityHash: null,
			entityId: signal.u,
			timestamp,
			score: signal.score,
			severity: signal.severity,
			signalType: signal.type,
			detectorsFired: 1,
			confidence: 0.5,
			detectorScores: [],
		};
	}

	async processAnomalyBatch(signals: IncomingAnomalySignal[]) {
		if (!signals || signals.length === 0) return;

		logger.info("Processing Tier-2 anomaly batch", { count: signals.length });

		const normalized = signals.map((signal) => this.normalizeSignal(signal));

		// Transform to Tier-2 storage payload.
		const events = normalized.map((sig) => {
			const context = `Anomaly detector=${sig.signalType} entity=${sig.entityId} score=${sig.score.toFixed(4)} severity=${sig.severity}`;
			return {
				textForEmbedding: context,
				payload: {
					entity_type: "anomaly",
					schema_version: sig.schemaVersion,
					entity_hash: sig.entityHash,
					entity_id: sig.entityId,
					start_ts: sig.timestamp,
					timestamp: sig.timestamp,
					score: sig.score,
					severity: sig.severity,
					signal_type: sig.signalType,
					detectors_fired: sig.detectorsFired,
					confidence: sig.confidence,
					detector_scores: sig.detectorScores,
					context,
				},
			};
		});

		await this.qdrant.ingestToTier2(events);

		const endTs = Math.floor(Date.now() / 1000);
		const startTs = endTs - 3600;
		await this.forensic.correlateIncidents(startTs, endTs);
	}
}
