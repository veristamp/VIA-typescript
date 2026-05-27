import { tier2IncidentGraphRepository } from "../modules/tier2/adapters/registry-repositories";
import type { Tier2IncidentGraphRepository } from "../modules/tier2/ports/repositories";
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
	groundTruthIds: Set<string>;
	benchmarkRunIds: Set<string>;
	firstSeenTs: number;
	lastSeenTs: number;
	severityMax: number;
	scoreMax: number;
	entityKey: string;
	evidence: Record<string, unknown>;
	reason: "temporal" | "semantic" | "trace";
	confidence: number;
}

const TEMPORAL_BUCKET_SECONDS = 300;

export class ForensicAnalysisService {
	constructor(
		private readonly qdrantService: QdrantService,
		private readonly incidentGraphRepository: Tier2IncidentGraphRepository = tier2IncidentGraphRepository,
	) {}

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
		const attrs = (payload.attributes || {}) as Record<string, unknown>;
		const groundTruthIdRaw = attrs.ground_truth_anomaly_id;
		const benchmarkRunIdRaw = attrs.benchmark_run_id;
		const ts = this.extractTs(payload);
		const severity = Number(payload.severity ?? 0);
		const score = Number(payload.score ?? hit.score ?? 0);
		const pointId = String(hit.id);

		const current = acc.get(incidentId);
		if (!current) {
			const groundTruthIds = new Set<string>();
			if (typeof groundTruthIdRaw === "string" && groundTruthIdRaw.length > 0) {
				groundTruthIds.add(groundTruthIdRaw);
			}
			const benchmarkRunIds = new Set<string>();
			if (
				typeof benchmarkRunIdRaw === "string" &&
				benchmarkRunIdRaw.length > 0
			) {
				benchmarkRunIds.add(benchmarkRunIdRaw);
			}
			acc.set(incidentId, {
				memberPointIds: new Set([pointId]),
				groundTruthIds,
				benchmarkRunIds,
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
		if (typeof groundTruthIdRaw === "string" && groundTruthIdRaw.length > 0) {
			current.groundTruthIds.add(groundTruthIdRaw);
		}
		if (typeof benchmarkRunIdRaw === "string" && benchmarkRunIdRaw.length > 0) {
			current.benchmarkRunIds.add(benchmarkRunIdRaw);
		}
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
		const temporalMinSupport = Math.max(2, Math.ceil(hits.length * 0.03));

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
			const bucket = Math.floor(ts / TEMPORAL_BUCKET_SECONDS);
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
			if (grouped.length < temporalMinSupport) continue;
			const incidentId = this.buildIncidentId(
				"temporal",
				bucket,
				Number(bucket) * TEMPORAL_BUCKET_SECONDS,
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
			const memberCount = value.memberPointIds.size;
			if (value.reason === "temporal" && memberCount < 2) {
				continue;
			}
			if (
				(value.reason === "semantic" || value.reason === "trace") &&
				memberCount < 2
			) {
				continue;
			}
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
				evidence: {
					...value.evidence,
					ground_truth_anomaly_ids: Array.from(value.groundTruthIds),
					benchmark_run_ids: Array.from(value.benchmarkRunIds),
				},
			});
		}

		return candidates;
	}

	private buildSeedHits(events: CanonicalTier2Event[]): QdrantScoredPoint[] {
		return events.map((event) => {
			const rhythmHashRaw = event.attributes.rhythm_hash;
			const rhythmHash =
				typeof rhythmHashRaw === "string" && rhythmHashRaw.length > 0
					? rhythmHashRaw
					: event.entityHash.slice(0, 16);
			return {
				id: event.eventId,
				score: event.score,
				payload: {
					event_id: event.eventId,
					entity_hash: event.entityHash,
					group_key: `${rhythmHash}:${event.primaryDetector}`,
					entity_id: event.entityId,
					start_ts: event.timestamp,
					timestamp: event.timestamp,
					score: event.score,
					severity: event.severity,
					signal_type: event.primaryDetector,
					detectors_fired: event.detectorsFired,
					confidence: event.confidence,
					detector_scores: event.detectorScores,
					attributes: event.attributes,
				},
			};
		});
	}

	async correlateIncidents(startTs: number, endTs: number): Promise<void> {
		const clusters = await this.qdrantService.findTier2Clusters(startTs, endTs);
		const candidates = this.buildCandidatesFromHits(clusters);

		for (const candidate of candidates) {
			for (const pointId of candidate.memberPointIds) {
				await this.incidentGraphRepository.saveIncidentGraph(
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
		_seedEvents: CanonicalTier2Event[] = [],
	): Promise<IncidentCandidate[]> {
		const clusters = await this.qdrantService.findTier2Clusters(startTs, endTs);
		const seedHits = this.buildSeedHits(_seedEvents);
		return this.buildCandidatesFromHits([...clusters, ...seedHits]);
	}

	async getIncidentGraph(metaIncidentId: string) {
		const graph =
			await this.incidentGraphRepository.getIncidentGraph(metaIncidentId);
		return graph;
	}
}
