import { tier2IncidentRepository } from "../modules/tier2/adapters/registry-repositories";
import { resolveIncidentDecision } from "../modules/tier2/domain/incident-decision";
import type { Tier2IncidentRepository } from "../modules/tier2/ports/repositories";
import type { IncidentCandidate, IncidentStatus } from "../types";
import { logger } from "../utils/logger";
import { Tier1SyncService, type Tier1FeedbackEvent } from "./tier1-sync-service";

export interface IncidentDecision {
	incidentId: string;
	status: IncidentStatus;
	reason: string;
	confidence: number;
	policyVersion: string;
}

const POLICY_VERSION = "tier2-policy-v1";

export class IncidentService {
	constructor(
		private readonly repository: Tier2IncidentRepository = tier2IncidentRepository,
		private readonly tier1Sync: Tier1SyncService = new Tier1SyncService(),
	) {}

	private parseEntityHashText(candidate: IncidentCandidate): string | undefined {
		const direct = candidate.entityKey.startsWith("hash:")
			? candidate.entityKey.slice("hash:".length)
			: "";
		if (/^\d+$/.test(direct)) {
			return direct;
		}
		const fromEvidence = candidate.evidence.entity_hash;
		if (typeof fromEvidence === "string" && /^\d+$/.test(fromEvidence)) {
			return fromEvidence;
		}
		return undefined;
	}

	private buildFeedbackEvent(
		candidate: IncidentCandidate,
		decision: IncidentDecision,
	): Tier1FeedbackEvent | null {
		let wasTruePositive: boolean;
		let labelClass: "true_positive" | "false_positive";
		if (decision.status === "suppressed") {
			wasTruePositive = false;
			labelClass = "false_positive";
		} else if (
			decision.status === "escalated" ||
			decision.status === "merged"
		) {
			wasTruePositive = true;
			labelClass = "true_positive";
		} else {
			return null;
		}

		const detectorScoresRaw = candidate.evidence.detector_scores;
		const detectorScores = Array.isArray(detectorScoresRaw)
			? detectorScoresRaw
					.map((value) => Number(value))
					.filter((value) => Number.isFinite(value))
			: [];
		const latencyMs = Math.max(
			0,
			Math.floor(Date.now() - candidate.lastSeenTs * 1000),
		);

		return {
			entity_hash_text: this.parseEntityHashText(candidate),
			entity_id: candidate.entityKey,
			signal_timestamp: candidate.lastSeenTs,
			was_true_positive: wasTruePositive,
			detector_scores: detectorScores,
			source: "tier2_auto",
			confidence: decision.confidence,
			label_class: labelClass,
			pattern_id: candidate.incidentId,
			feedback_latency_ms: latencyMs,
		};
	}

	private resolveDecision(candidate: IncidentCandidate): IncidentDecision {
		const resolved = resolveIncidentDecision(candidate);

		return {
			incidentId: candidate.incidentId,
			status: resolved.status,
			reason: `reason=${candidate.reason};members=${candidate.memberPointIds.length};severity_max=${candidate.severityMax.toFixed(3)};score_max=${candidate.scoreMax.toFixed(3)}`,
			confidence: resolved.confidence,
			policyVersion: POLICY_VERSION,
		};
	}

	async applyCandidates(
		candidates: IncidentCandidate[],
	): Promise<IncidentDecision[]> {
		const decisions: IncidentDecision[] = [];
		const feedbackEvents: Tier1FeedbackEvent[] = [];

		for (const candidate of candidates) {
			const decision = this.resolveDecision(candidate);
			const confidencePct = Math.round(decision.confidence * 100);
			await this.repository.upsertIncident({
				incidentId: decision.incidentId,
				status: decision.status,
				entityKey: candidate.entityKey,
				firstSeenTs: candidate.firstSeenTs,
				lastSeenTs: candidate.lastSeenTs,
				severityMaxPct: Math.round(candidate.severityMax * 100),
				scoreMaxPct: Math.round(candidate.scoreMax * 100),
				confidencePct,
				evidence: {
					...candidate.evidence,
					member_point_ids: candidate.memberPointIds,
					reason: candidate.reason,
				},
				policyVersion: decision.policyVersion,
			});
			await this.repository.saveDecision(
				decision.incidentId,
				decision.status,
				decision.reason,
				confidencePct,
				decision.policyVersion,
			);
			decisions.push(decision);
			const feedbackEvent = this.buildFeedbackEvent(candidate, decision);
			if (feedbackEvent) {
				feedbackEvents.push(feedbackEvent);
			}
		}

		if (feedbackEvents.length > 0) {
			await this.tier1Sync.sendFeedback(feedbackEvents);
		}

		if (decisions.length > 0) {
			logger.info("Applied incident decisions", { count: decisions.length });
		}

		return decisions;
	}

	async listIncidents(limit: number): Promise<unknown[]> {
		return this.repository.listIncidents(limit);
	}

	async listIncidentsForRun(runId: string, limit: number): Promise<unknown[]> {
		return this.repository.listIncidentsForRun(runId, limit);
	}

	async getIncident(incidentId: string): Promise<unknown> {
		const incident = await this.repository.getIncidentById(incidentId);
		if (!incident) {
			return null;
		}
		const decisions = await this.repository.listDecisions(incidentId, 50);
		return {
			incident,
			decisions,
		};
	}
}
