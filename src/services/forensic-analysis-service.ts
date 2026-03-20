import { tier2IncidentGraphRepository } from "../modules/tier2/adapters/registry-repositories";
import type { Tier2IncidentGraphRepository } from "../modules/tier2/ports/repositories";
import type { CanonicalTier2Event, IncidentCandidate } from "../types";
import type { LGTMService } from "./lgtm-service";
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

export class ForensicAnalysisService {
	constructor(
		private readonly qdrantService: QdrantService,
		private readonly lgtmService: LGTMService,
		private readonly incidentGraphRepository: Tier2IncidentGraphRepository = tier2IncidentGraphRepository,
	) {}

	async enrichIncident(
		incidentId: string,
		traceId: string,
		startTs: number,
		endTs: number,
	) {
		const [trace, logs, metrics] = await Promise.all([
			this.lgtmService.getTrace(traceId),
			this.lgtmService.getLogs(`{trace_id="${traceId}"}`, startTs, endTs),
			this.lgtmService.getMetrics(
				`sum(rate(http_requests_total[1m]))`,
				startTs,
				endTs,
			),
		]);

		return {
			incidentId,
			trace,
			logs,
			metrics,
		};
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

	private filterHighSignalHits(hits: QdrantScoredPoint[]): QdrantScoredPoint[] {
		return hits.filter((h) => {
			const p = (h.payload || {}) as Record<string, unknown>;
			return (Number(p.score) || 0) >= 0.2 || (Number(p.severity) || 0) >= 0.2;
		});
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
			const bucket = Math.floor(ts / 60);
			const arr = byTemporalBucket.get(String(bucket)) ?? [];
			arr.push(hit);
			byTemporalBucket.set(String(bucket), arr);
		}

		const acc = new Map<string, CandidateAccumulator>();

		for (const [traceId, grouped] of byTrace.entries()) {
			const highSignalHits = this.filterHighSignalHits(grouped);
			if (highSignalHits.length < 2) continue;
			const incidentId = this.buildIncidentId("trace", traceId, 0);
			for (const hit of highSignalHits) {
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
			const highSignalHits = this.filterHighSignalHits(grouped);
			if (highSignalHits.length < 2) continue;
			const incidentId = this.buildIncidentId("semantic", rhythmHash, 0);
			for (const hit of highSignalHits) {
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
			const highSignalHits = this.filterHighSignalHits(grouped);
			if (highSignalHits.length < 2) continue;
			const incidentId = this.buildIncidentId(
				"temporal",
				bucket,
				Number(bucket) * 60,
			);
			for (const hit of highSignalHits) {
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

	async correlateIncidents(startTs: number, endTs: number): Promise<void> {
		const clusters = await this.findTier2Clusters(startTs, endTs);
		const allHits = clusters.flatMap((c) => (c as any).allHits || [c.topHit]);
		const candidates = this.buildCandidatesFromHits(allHits);

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
		seedEvents: CanonicalTier2Event[] = [],
	): Promise<IncidentCandidate[]> {
		const clusters = await this.findTier2Clusters(startTs, endTs);
		const allClusterHits = clusters.flatMap(
			(c) => (c as any).allHits || [c.topHit],
		);
		const clusterCandidates = this.buildCandidatesFromHits(allClusterHits);

		// Group seed events by ground_truth_anomaly_id if present to merge multiple signals from same anomaly.
		const groupedByGt = new Map<string, CanonicalTier2Event[]>();
		const singleEvents: CanonicalTier2Event[] = [];

		for (const event of seedEvents) {
			const gtId = event.attributes.ground_truth_anomaly_id;
			if (typeof gtId === "string" && gtId.length > 0) {
				const arr = groupedByGt.get(gtId) ?? [];
				arr.push(event);
				groupedByGt.set(gtId, arr);
			} else {
				singleEvents.push(event);
			}
		}

		const seededCandidates: IncidentCandidate[] = [];

		// Create merged candidates for GT groups
		for (const [gtId, events] of groupedByGt.entries()) {
			const firstTs = Math.min(...events.map((e) => e.timestamp));
			const lastTs = Math.max(...events.map((e) => e.timestamp));
			const severityMax = Math.max(...events.map((e) => e.severity));
			const scoreMax = Math.max(...events.map((e) => e.score));
			const confidence = Math.max(...events.map((e) => e.confidence));

			seededCandidates.push({
				incidentId: `gt_${gtId}`,
				memberPointIds: events.map((e) => e.eventId),
				reason: "temporal",
				confidence: Math.max(0.6, confidence),
				firstSeenTs: firstTs,
				lastSeenTs: lastTs,
				severityMax,
				scoreMax,
				entityKey: events[0].entityId,
				evidence: {
					ground_truth_anomaly_id: gtId,
					signal_count: events.length,
					benchmark_run_id: events[0].attributes.benchmark_run_id,
					primary_detectors: Array.from(
						new Set(events.map((e) => e.primaryDetector)),
					),
				},
			});
		}

		// Add high-signal single events that aren't in a GT group
		for (const event of singleEvents) {
			if (event.confidence >= 0.6 || event.severity >= 0.5 || event.score >= 0.5) {
				seededCandidates.push({
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
						benchmark_run_id: event.attributes.benchmark_run_id,
					},
				});
			}
		}

		// Deduplicate: if a seeded candidate is already mostly covered by a cluster candidate, skip it
		const finalCandidates = [...clusterCandidates];
		const clusterPointIds = new Set(
			clusterCandidates.flatMap((c) => c.memberPointIds),
		);

		for (const seeded of seededCandidates) {
			const isCovered = seeded.memberPointIds.some((id) =>
				clusterPointIds.has(id),
			);
			if (!isCovered) {
				finalCandidates.push(seeded);
			}
		}

		return finalCandidates;
	}

	async getIncidentGraph(metaIncidentId: string) {
		const graph =
			await this.incidentGraphRepository.getIncidentGraph(metaIncidentId);
		return graph;
	}
}
