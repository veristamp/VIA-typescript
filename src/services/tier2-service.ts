import type { Tier1AnomalySignalV1 } from "../modules/tier2/contracts/tier1-signal";
import type { CanonicalTier2Event } from "../types";
import { logger } from "../utils/logger";
import type { ForensicAnalysisService } from "./forensic-analysis-service";
import type { IncidentService } from "./incident-service";
import type { QdrantService } from "./qdrant-service";

export type IncomingAnomalySignal = Tier1AnomalySignalV1;

export class Tier2Service {
	constructor(
		private qdrant: QdrantService,
		private forensic: ForensicAnalysisService,
		private incidents: IncidentService,
	) {}

	private normalizeSignal(signal: IncomingAnomalySignal): CanonicalTier2Event {
		return {
			eventId: signal.event_id,
			schemaVersion: signal.schema_version,
			entityHash: signal.entity_hash,
			entityId: `hash:${signal.entity_hash}`,
			timestamp: signal.timestamp,
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
			const rhythmHashRaw = sig.attributes.rhythm_hash;
			const rhythmHash =
				typeof rhythmHashRaw === "string" && rhythmHashRaw.length > 0
					? rhythmHashRaw
					: sig.entityHash.slice(0, 16);
			const groupKey = `${rhythmHash}:${sig.primaryDetector}`;
			const context = `rhythm=${rhythmHash} det=${sig.primaryDetector}`;
			return {
				textForEmbedding: context,
				payload: {
					event_id: sig.eventId,
					entity_type: "anomaly",
					schema_version: sig.schemaVersion,
					entity_hash: sig.entityHash,
					rhythm_hash: rhythmHash,
					group_key: groupKey,
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

		const timestamps = normalized.map((e) => e.timestamp);
		const endTs = Math.max(...timestamps);
		const startTs = Math.min(...timestamps);
		const candidates = await this.forensic.deriveIncidentCandidates(
			startTs,
			endTs,
			normalized,
		);
		await this.incidents.applyCandidates(candidates);
	}
}
