import { getIncidentGraph, saveIncidentGraph } from "../db/registry";
import type { QdrantService } from "./qdrant-service";

export interface ClusterResult {
	clusterId: string | number;
	incidentCount: number;
	topHit: {
		id: string | number;
		payload: Record<string, unknown>;
	};
}

export interface TriageResult {
	id: string | number;
	score: number;
	payload: Record<string, unknown>;
}

export class ForensicAnalysisService {
	private qdrantService: QdrantService;

	constructor(qdrantService: QdrantService) {
		this.qdrantService = qdrantService;
	}

	async findTier2Clusters(
		startTs: number,
		endTs: number,
		textFilter?: string,
	): Promise<ClusterResult[]> {
		const clusters = await this.qdrantService.findTier2Clusters(
			startTs,
			endTs,
			textFilter,
		);
		return clusters.map((cluster) => {
			const payload = (cluster.payload || {}) as Record<string, unknown>;
			return {
				clusterId: cluster.id,
				incidentCount: Number(payload.count) || 1,
				topHit: {
					id: cluster.id,
					payload,
				},
			};
		});
	}

	async triageSimilarEvents(
		positiveIds: string[],
		negativeIds: string[],
		startTs: number,
		endTs: number,
	): Promise<TriageResult[]> {
		const results = await this.qdrantService.triageSimilarEvents(
			positiveIds,
			negativeIds,
			startTs,
			endTs,
		);

		return results.map((result) => ({
			id: result.id,
			score: result.score,
			payload: (result.payload || {}) as Record<string, unknown>,
		}));
	}

	async correlateIncidents(startTs: number, endTs: number): Promise<void> {
		const clusters = await this.qdrantService.findTier2Clusters(startTs, endTs);

		for (let i = 0; i < clusters.length; i++) {
			const cluster1 = clusters[i];
			const payload1 = (cluster1.payload || {}) as Record<string, unknown>;
			const attributes1 = (payload1.attributes || {}) as Record<
				string,
				unknown
			>;
			const traceId1 = attributes1.trace_id || attributes1.traceId;

			for (let j = i + 1; j < clusters.length; j++) {
				const cluster2 = clusters[j];
				const payload2 = (cluster2.payload || {}) as Record<string, unknown>;
				const attributes2 = (payload2.attributes || {}) as Record<
					string,
					unknown
				>;
				const traceId2 = attributes2.trace_id || attributes2.traceId;

				// 1. Temporal Link
				const ts1 = Number(payload1?.start_ts ?? payload1?.timestamp ?? 0);
				const ts2 = Number(payload2?.start_ts ?? payload2?.timestamp ?? 0);
				const timeDiff = Math.abs(
					(Number.isFinite(ts1) ? ts1 : 0) - (Number.isFinite(ts2) ? ts2 : 0),
				);

				if (timeDiff < 3600) {
					await saveIncidentGraph(
						`meta_incident_${i}_${j}`,
						String(cluster1.id),
						"temporal",
						80,
					);
				}

				// 2. Semantic Link (Rhythm Hash)
				const rhythmHash1 = payload1?.rhythm_hash;
				const rhythmHash2 = payload2?.rhythm_hash;

				if (rhythmHash1 && rhythmHash1 === rhythmHash2) {
					await saveIncidentGraph(
						`meta_incident_${i}_${j}`,
						String(cluster2.id),
						"semantic",
						85,
					);
				}

				// 3. Trace Link (Trace ID)
				if (traceId1 && traceId2 && traceId1 === traceId2) {
					await saveIncidentGraph(
						`meta_incident_${i}_${j}`,
						String(cluster2.id),
						"trace",
						100, // High confidence for explicit trace correlation
					);
				}
			}
		}
	}

	async getIncidentGraph(metaIncidentId: string) {
		const graph = await getIncidentGraph(metaIncidentId);
		return graph;
	}
}
