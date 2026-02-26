import { tier2IncidentRepository } from "../modules/tier2/adapters/registry-repositories";
import { resolveIncidentDecision } from "../modules/tier2/domain/incident-decision";
import type { Tier2IncidentRepository } from "../modules/tier2/ports/repositories";
import type { IncidentCandidate, IncidentStatus } from "../types";
import { logger } from "../utils/logger";

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
	) {}

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
