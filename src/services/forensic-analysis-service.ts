import * as registry from "../db/registry";
import type { CanonicalTier2Event, IncidentCandidate } from "../types";
import { logger } from "../utils/logger";
import type { QdrantScoredPoint, QdrantService } from "./qdrant-service";

export interface ClusterResult {
	clusterId: string | number;
	incidentCount: number;
	topHit: {
		id: string | number;
		payload: Record<string, unknown>;
	};
}

interface CandidateAccumulator {
	incidentId: string;
	memberPointIds: Set<string>;
	reason: "temporal" | "semantic" | "trace";
	confidence: number;
	firstSeenTs: number;
	lastSeenTs: number;
	severityMax: number;
	scoreMax: number;
	entityKey: string;
	evidence: Record<string, unknown>;
}

export class ForensicAnalysisService {
	constructor(
		private qdrantService: QdrantService,
	) {}

	private filterHighSignalHits(hits: QdrantScoredPoint[]): QdrantScoredPoint[] {
		return hits.filter((h) => {
			const p = h.payload as any;
			return p?.confidence >= 0.4 || p?.severity >= 0.4 || p?.score >= 0.4;
		});
	}

	private accumulate(
		acc: Map<string, CandidateAccumulator>,
		incidentId: string,
		hit: QdrantScoredPoint,
		reason: "temporal" | "semantic" | "trace",
		confidence: number,
		entityKey: string,
		evidence: Record<string, unknown>,
	) {
		const payload = (hit.payload || {}) as Record<string, unknown>;
		const ts = this.extractTs(payload);
		const severity = Number(payload.severity ?? 0);
		const score = Number(payload.score ?? hit.score ?? 0);
		const pointId = String(hit.id);

		const current = acc.get(incidentId);
		if (!current) {
			const gtId = payload.attributes ? (payload.attributes as any).ground_truth_anomaly_id : payload.ground_truth_anomaly_id;
			const runId = payload.attributes ? (payload.attributes as any).benchmark_run_id : payload.benchmark_run_id;

			acc.set(incidentId, {
				incidentId,
				memberPointIds: new Set([pointId]),
				reason,
				confidence,
				firstSeenTs: ts,
				lastSeenTs: ts,
				severityMax: severity,
				scoreMax: score,
				entityKey,
				evidence: {
					...evidence,
					initial_detector: payload.primary_detector,
					ground_truth_anomaly_id: gtId,
					benchmark_run_id: runId,
				},
			});
			return;
		}

		current.memberPointIds.add(pointId);
		
		const hitGtId = payload.attributes ? (payload.attributes as any).ground_truth_anomaly_id : payload.ground_truth_anomaly_id;
		if (hitGtId && current.evidence.ground_truth_anomaly_id !== hitGtId) {
			const ids = new Set(Array.isArray(current.evidence.ground_truth_anomaly_ids) ? current.evidence.ground_truth_anomaly_ids : []);
			if (current.evidence.ground_truth_anomaly_id) ids.add(current.evidence.ground_truth_anomaly_id as string);
			ids.add(hitGtId);
			current.evidence.ground_truth_anomaly_ids = Array.from(ids);
		}
		current.confidence = Math.max(current.confidence, confidence);
		current.firstSeenTs = Math.min(current.firstSeenTs, ts);
		current.lastSeenTs = Math.max(current.lastSeenTs, ts);
		current.severityMax = Math.max(
			current.severityMax,
			Number.isFinite(severity) ? severity : 0,
		);
		current.scoreMax = Math.max(
			current.scoreMax,
			Number.isFinite(score) ? score : 0,
		);
	}

	private buildCandidatesFromHits(
		hits: QdrantScoredPoint[],
	): IncidentCandidate[] {
		const acc = new Map<string, CandidateAccumulator>();

		// Group by logical group_key
		const byGroup = new Map<string, QdrantScoredPoint[]>();
		for (const hit of hits) {
			const payload = (hit.payload || {}) as Record<string, unknown>;
			const key = String(payload.group_key || hit.id);
			const arr = byGroup.get(key) ?? [];
			arr.push(hit);
			byGroup.set(key, arr);
		}

		for (const [groupKey, groupHits] of byGroup.entries()) {
			const highSignalHits = this.filterHighSignalHits(groupHits);
			if (highSignalHits.length === 0) continue;

			// Sort by timestamp to detect temporal gaps
			highSignalHits.sort((a, b) => this.extractTs(a.payload as any) - this.extractTs(b.payload as any));

			let currentSubGroup: QdrantScoredPoint[] = [highSignalHits[0]];
			
			for (let i = 1; i < highSignalHits.length; i++) {
				const prevTs = this.extractTs(highSignalHits[i - 1].payload as any);
				const currTs = this.extractTs(highSignalHits[i].payload as any);
				
				// 10-minute gap means a new incident for the same pattern
				if (currTs - prevTs > 600) {
					this.processSubGroup(acc, groupKey, currentSubGroup);
					currentSubGroup = [highSignalHits[i]];
				} else {
					currentSubGroup.push(highSignalHits[i]);
				}
			}
			this.processSubGroup(acc, groupKey, currentSubGroup);
		}

		const candidates: IncidentCandidate[] = [];
		for (const [incidentId, value] of acc.entries()) {
			if (value.memberPointIds.size < 5) continue;

			candidates.push({
				incidentId,
				memberPointIds: Array.from(value.memberPointIds),
				reason: value.reason,
				confidence: value.confidence,
				firstSeenTs: value.firstSeenTs,
				lastSeenTs: value.lastSeenTs,
				severityMax: value.severityMax,
				scoreMax: value.scoreMax,
				entityKey: value.entityKey,
				evidence: value.evidence,
			});
		}

		return candidates;
	}

	private processSubGroup(
		acc: Map<string, CandidateAccumulator>,
		groupKey: string,
		hits: QdrantScoredPoint[]
	) {
		const sample = hits[0].payload as any;
		const reason = sample?.rhythm_hash ? "semantic" : "temporal";
		const entityKey = String(sample?.entity_id || sample?.entity_hash || groupKey);
		
		// Use the first timestamp of the subgroup as an anchor for stability
		const anchorTs = this.extractTs(sample);
		const timeAnchor = Math.floor(anchorTs / 3600); // Hourly stability
		
		const incidentId = this.buildIncidentId(reason, groupKey, timeAnchor);

		for (const hit of hits) {
			this.accumulate(acc, incidentId, hit, reason, 0.8, entityKey, { group_key: groupKey });
		}
	}

	async findTier2Clusters(
		startTs: number,
		endTs: number,
		textFilter?: string,
	): Promise<ClusterResult[]> {
		const hits = await this.qdrantService.findTier2Clusters(
			startTs,
			endTs,
			textFilter,
		);

		const grouped = new Map<string, QdrantScoredPoint[]>();
		for (const hit of hits) {
			const key = String(hit.payload?.group_key ?? hit.id);
			const arr = grouped.get(key) ?? [];
			arr.push(hit);
			grouped.set(key, arr);
		}

		return Array.from(grouped.entries()).map(([key, groupHits]) => {
			const topHit = groupHits[0];
			const payload = (topHit.payload || {}) as Record<string, unknown>;
			return {
				clusterId: key,
				incidentCount: groupHits.length,
				topHit: {
					id: topHit.id,
					payload,
				},
				allHits: groupHits,
			};
		});
	}

	async triageSimilarEvents(
		metaIncidentId: string,
		targetEvent: CanonicalTier2Event,
	) {
		const similar = await this.qdrantService.triageSimilarEvents(
			targetEvent.textForEmbedding || `rhythm=${targetEvent.attributes.rhythm_hash}`,
			targetEvent.timestamp - 300,
			targetEvent.timestamp + 300,
		);

		for (const hit of similar) {
			await registry.saveIncidentGraph(
				metaIncidentId,
				String(hit.id),
				"semantic",
				Math.round((hit.score || 0) * 100),
			);
		}
	}

	async correlateIncidents(startTs: number, endTs: number): Promise<void> {
		const clusters = await this.findTier2Clusters(startTs, endTs);
		const allHits = clusters.flatMap((c) => (c as any).allHits || [c.topHit]);
		const candidates = this.buildCandidatesFromHits(allHits);

		for (const candidate of candidates) {
			for (const pointId of candidate.memberPointIds) {
				await registry.saveIncidentGraph(
					candidate.incidentId,
					pointId,
					candidate.reason,
					Math.round(candidate.confidence * 100),
				);
			}
		}
	}

	async deriveIncidentCandidates(
		startTs: number,
		endTs: number,
		seedEvents: CanonicalTier2Event[] = [],
	): Promise<IncidentCandidate[]> {
		// Look back 15 minutes for better continuity
		const clusters = await this.findTier2Clusters(startTs - 900, endTs);
		const allClusterHits = clusters.flatMap(
			(c) => (c as any).allHits || [c.topHit],
		);
		
		const seedHits: QdrantScoredPoint[] = seedEvents.map(e => {
			const rhythmHash = (e.attributes.rhythm_hash as string) || `det_${e.primaryDetector}`;
			return {
				id: e.eventId,
				score: e.score,
				payload: {
					...e,
					group_key: rhythmHash,
				}
			};
		});

		const allHits = [...allClusterHits, ...seedHits];
		const finalCandidates = this.buildCandidatesFromHits(allHits);

		return finalCandidates;
	}

	private buildIncidentId(
		reason: string,
		key: string,
		anchor: number,
	): string {
		const raw = `${reason}:${key}:${anchor}`;
		return `inc_${Bun.hash.xxHash64(raw).toString(16)}`;
	}

	private extractTs(payload: Record<string, unknown>): number {
		const ts = Number(payload.start_ts ?? payload.timestamp ?? 0);
		return Number.isFinite(ts) ? ts : Math.floor(Date.now() / 1000);
	}

	async getIncidentGraph(metaIncidentId: string) {
		const graph =
			await registry.getIncidentGraph(metaIncidentId);
		return graph;
	}
}
