import { getIncidentGraph, saveIncidentGraph } from "../db/registry";
import type { CanonicalTier2Event, IncidentCandidate } from "../types";
import type { QdrantScoredPoint, QdrantService } from "./qdrant-service";

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

interface CandidateAccumulator {
	memberPointIds: Set<string>;
	firstSeenTs: number;
	lastSeenTs: number;
	severityMax: number;
	scoreMax: number;
	entityKey: string;
	evidence: Record<string, unknown>;
	reason: "temporal" | "semantic" | "trace";
	confidence: number;
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

	private buildIncidentId(
		reason: string,
		key: string,
		bucketTs: number,
	): string {
		const raw = `${reason}:${key}:${bucketTs}`;
		return `inc_${Bun.hash.xxHash64(raw).toString(16)}`;
	}

	private extractTs(payload: Record<string, unknown>): number {
		const ts = Number(payload.start_ts ?? payload.timestamp ?? 0);
		return Number.isFinite(ts) ? ts : 0;
	}

	private accumulate(
		acc: Map<string, CandidateAccumulator>,
		incidentId: string,
		hit: QdrantScoredPoint,
		reason: "temporal" | "semantic" | "trace",
		confidence: number,
		entityKey: string,
		seedEvidence: Record<string, unknown>,
	): void {
		const payload = (hit.payload || {}) as Record<string, unknown>;
		const ts = this.extractTs(payload);
		const severity = Number(payload.severity ?? 0);
		const score = Number(payload.score ?? hit.score ?? 0);
		const pointId = String(hit.id);

		const current = acc.get(incidentId);
		if (!current) {
			acc.set(incidentId, {
				memberPointIds: new Set([pointId]),
				firstSeenTs: ts,
				lastSeenTs: ts,
				severityMax: Number.isFinite(severity) ? severity : 0,
				scoreMax: Number.isFinite(score) ? score : 0,
				entityKey,
				evidence: seedEvidence,
				reason,
				confidence,
			});
			return;
		}

		current.memberPointIds.add(pointId);
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
		const byTrace = new Map<string, QdrantScoredPoint[]>();
		const byRhythm = new Map<string, QdrantScoredPoint[]>();
		const byTemporalBucket = new Map<string, QdrantScoredPoint[]>();

		for (const hit of hits) {
			const payload = (hit.payload || {}) as Record<string, unknown>;
			const attrs = (payload.attributes || {}) as Record<string, unknown>;
			const traceIdRaw = attrs.trace_id ?? attrs.traceId;
			if (typeof traceIdRaw === "string" && traceIdRaw.length > 0) {
				const arr = byTrace.get(traceIdRaw) ?? [];
				arr.push(hit);
				byTrace.set(traceIdRaw, arr);
			}

			const rhythmHash = payload.rhythm_hash;
			if (typeof rhythmHash === "string" && rhythmHash.length > 0) {
				const arr = byRhythm.get(rhythmHash) ?? [];
				arr.push(hit);
				byRhythm.set(rhythmHash, arr);
			}

			const ts = this.extractTs(payload);
			const bucket = Math.floor(ts / 300);
			const arr = byTemporalBucket.get(String(bucket)) ?? [];
			arr.push(hit);
			byTemporalBucket.set(String(bucket), arr);
		}

		const acc = new Map<string, CandidateAccumulator>();

		for (const [traceId, grouped] of byTrace.entries()) {
			if (grouped.length < 2) continue;
			const incidentId = this.buildIncidentId("trace", traceId, 0);
			for (const hit of grouped) {
				this.accumulate(
					acc,
					incidentId,
					hit,
					"trace",
					1.0,
					`trace:${traceId}`,
					{ trace_id: traceId },
				);
			}
		}

		for (const [rhythmHash, grouped] of byRhythm.entries()) {
			if (grouped.length < 2) continue;
			const incidentId = this.buildIncidentId("semantic", rhythmHash, 0);
			for (const hit of grouped) {
				this.accumulate(
					acc,
					incidentId,
					hit,
					"semantic",
					0.85,
					`rhythm:${rhythmHash}`,
					{ rhythm_hash: rhythmHash },
				);
			}
		}

		for (const [bucket, grouped] of byTemporalBucket.entries()) {
			if (grouped.length < 2) continue;
			const incidentId = this.buildIncidentId(
				"temporal",
				bucket,
				Number(bucket) * 300,
			);
			for (const hit of grouped) {
				this.accumulate(
					acc,
					incidentId,
					hit,
					"temporal",
					0.8,
					`bucket:${bucket}`,
					{ temporal_bucket: bucket },
				);
			}
		}

		const candidates: IncidentCandidate[] = [];
		for (const [incidentId, value] of acc.entries()) {
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

	async correlateIncidents(startTs: number, endTs: number): Promise<void> {
		const clusters = await this.qdrantService.findTier2Clusters(startTs, endTs);
		const candidates = this.buildCandidatesFromHits(clusters);

		for (const candidate of candidates) {
			for (const pointId of candidate.memberPointIds) {
				await saveIncidentGraph(
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
		const clusters = await this.qdrantService.findTier2Clusters(startTs, endTs);
		const clusterCandidates = this.buildCandidatesFromHits(clusters);

		// Seed single-event candidates so new events always show up in workflow.
		const seededCandidates: IncidentCandidate[] = seedEvents.map((event) => ({
			incidentId: `evt_${event.eventId}`,
			memberPointIds: [event.eventId],
			reason: "temporal",
			confidence: Math.max(0.4, Math.min(1, event.confidence)),
			firstSeenTs: event.timestamp,
			lastSeenTs: event.timestamp,
			severityMax: event.severity,
			scoreMax: event.score,
			entityKey: event.entityId,
			evidence: {
				event_id: event.eventId,
				primary_detector: event.primaryDetector,
			},
		}));

		return [...clusterCandidates, ...seededCandidates];
	}

	async getIncidentGraph(metaIncidentId: string) {
		const graph = await getIncidentGraph(metaIncidentId);
		return graph;
	}
}
