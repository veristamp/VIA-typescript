import type { CanonicalTier2Event } from "../types";
import { logger } from "../utils/logger";
import type { ForensicAnalysisService } from "./forensic-analysis-service";
import type { IncidentService } from "./incident-service";
import type { QdrantService } from "./qdrant-service";

export interface Tier1AnomalySignalV1 {
	event_id: string;
	schema_version: number;
	tenant_id: string;
	entity_hash: number;
	timestamp: number;
	score: number;
	severity: number;
	primary_detector: number;
	detectors_fired: number;
	confidence: number;
	detector_scores: number[];
	attributes?: Record<string, unknown>;
}

export type IncomingAnomalySignal = Tier1AnomalySignalV1;

export class Tier2Service {
	constructor(
		private qdrant: QdrantService,
		private forensic: ForensicAnalysisService,
		private incidents: IncidentService,
	) {}

	private normalizeToUnixSeconds(ts: number): number {
		if (!Number.isFinite(ts) || ts <= 0) {
			return Math.floor(Date.now() / 1000);
		}
		if (ts > 1e15) return Math.floor(ts / 1e9);
		if (ts > 1e12) return Math.floor(ts / 1e3);
		return Math.floor(ts);
	}

	private normalizeSignal(signal: IncomingAnomalySignal): CanonicalTier2Event {
		const timestamp = this.normalizeToUnixSeconds(signal.timestamp);
		return {
			eventId: signal.event_id,
			schemaVersion: signal.schema_version,
			tenantId: signal.tenant_id,
			entityHash: signal.entity_hash,
			entityId: `hash:${signal.entity_hash}`,
			timestamp,
			score: signal.score,
			severity: signal.severity,
			primaryDetector: signal.primary_detector,
			detectorsFired: signal.detectors_fired,
			confidence: signal.confidence,
			detectorScores: signal.detector_scores,
			attributes: signal.attributes ?? {},
		};
	}

	deriveBatchEventId(signals: IncomingAnomalySignal[]): string {
		const normalized = signals.map((signal) => this.normalizeSignal(signal));
		const seed = normalized
			.map((event) => `${event.eventId}:${event.timestamp}`)
			.sort()
			.join("|");
		return Bun.hash.xxHash64(seed || String(Date.now())).toString(16);
	}

	async processAnomalyBatch(signals: IncomingAnomalySignal[]): Promise<void> {
		if (!signals || signals.length === 0) return;

		const normalized = signals.map((signal) => this.normalizeSignal(signal));
		logger.info("Processing Tier-2 canonical anomaly batch", {
			count: normalized.length,
		});

		const events = normalized.map((sig) => {
			const context = `anomaly event=${sig.eventId} detector=${sig.primaryDetector} entity=${sig.entityId} score=${sig.score.toFixed(4)} severity=${sig.severity.toFixed(4)}`;
			return {
				textForEmbedding: context,
				payload: {
					event_id: sig.eventId,
					entity_type: "anomaly",
					schema_version: sig.schemaVersion,
					tenant_id: sig.tenantId,
					entity_hash: sig.entityHash,
					entity_id: sig.entityId,
					start_ts: sig.timestamp,
					timestamp: sig.timestamp,
					score: sig.score,
					severity: sig.severity,
					signal_type: sig.primaryDetector,
					detectors_fired: sig.detectorsFired,
					confidence: sig.confidence,
					detector_scores: sig.detectorScores,
					attributes: sig.attributes,
					context,
				},
			};
		});

		await this.qdrant.ingestToTier2(events);

		const endTs = Math.floor(Date.now() / 1000);
		const startTs = endTs - 3600;
		const candidates = await this.forensic.deriveIncidentCandidates(
			startTs,
			endTs,
			normalized,
		);
		await this.incidents.seedSingleEventIncident(normalized);
		await this.incidents.applyCandidates(candidates);
	}
}
