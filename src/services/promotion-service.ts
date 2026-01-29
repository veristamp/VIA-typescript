import type { AnomalySignal } from "../core/anomaly-profile";
import type { QdrantService, Tier2Event } from "./qdrant-service";

export interface AnomalyCluster {
	rhythmHash: string;
	service: string;
	count: number;
	anomalies: AnomalySignal[];
}

export class PromotionService {
	private qdrantService: QdrantService;

	constructor(qdrantService: QdrantService) {
		this.qdrantService = qdrantService;
	}

	async promoteAnomalies(anomalies: AnomalySignal[]): Promise<void> {
		if (!anomalies || anomalies.length === 0) {
			return;
		}

		// Group anomalies by rhythm_hash
		const clusters = new Map<string, AnomalyCluster>();
		for (const anomaly of anomalies) {
			const hash = anomaly.rhythmHash;
			if (!clusters.has(hash)) {
				clusters.set(hash, {
					rhythmHash: hash,
					service: anomaly.service,
					count: 1,
					anomalies: [anomaly],
				});
			}
		}

		// Transform to Tier-2 events
		const events: Tier2Event[] = [];
		for (const [hash, cluster] of clusters.entries()) {
			const sortedAnomalies = [...cluster.anomalies].sort(
				(a, b) => a.timestamp - b.timestamp,
			);

			const startTs = sortedAnomalies[0].timestamp;
			const endTs = sortedAnomalies[sortedAnomalies.length - 1].timestamp;
			const textForEmbedding = sortedAnomalies[0].context || "";

			const eventPayload = {
				entity_type: "event_cluster",
				rhythm_hash: hash,
				start_ts: startTs,
				end_ts: endTs,
				count: cluster.count,
				service: cluster.service,
				severity: sortedAnomalies[0].severity,
				anomaly_type: sortedAnomalies[0].anomalyType,
				anomaly_context: sortedAnomalies[0].context,
				body: textForEmbedding,
				sample_logs: sortedAnomalies
					.slice(0, 5)
					.map((a) => a.metadata?.fullLogJson || null),
			};

			events.push({
				textForEmbedding,
				payload: eventPayload,
			});
		}

		// Ingest to Tier-2
		await this.qdrantService.ingestToTier2(events);
	}
}
