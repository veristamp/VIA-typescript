import {
	getTier2IncidentById,
	listTier2Decisions,
	listTier2Incidents,
	saveTier2Decision,
	upsertTier2Incident,
} from "../db/registry";
import type {
	CanonicalTier2Event,
	IncidentCandidate,
	IncidentStatus,
} from "../types";
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
	private resolveDecision(candidate: IncidentCandidate): IncidentDecision {
		const confidence = Math.max(0, Math.min(1, candidate.confidence));
		const status: IncidentStatus =
			candidate.severityMax >= 0.9 || candidate.scoreMax >= 0.95
				? "escalated"
				: candidate.memberPointIds.length >= 3 && confidence >= 0.8
					? "merged"
					: "new";

		return {
			incidentId: candidate.incidentId,
			status,
			reason: `reason=${candidate.reason};members=${candidate.memberPointIds.length};severity_max=${candidate.severityMax.toFixed(3)};score_max=${candidate.scoreMax.toFixed(3)}`,
			confidence,
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
			await upsertTier2Incident({
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
			await saveTier2Decision(
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

	async seedSingleEventIncident(events: CanonicalTier2Event[]): Promise<void> {
		for (const event of events) {
			const incidentId = `evt_${event.eventId}`;
			const existing = await getTier2IncidentById(incidentId);
			if (existing) {
				continue;
			}
			await upsertTier2Incident({
				incidentId,
				status: "new",
				entityKey: event.entityId,
				firstSeenTs: event.timestamp,
				lastSeenTs: event.timestamp,
				severityMaxPct: Math.round(event.severity * 100),
				scoreMaxPct: Math.round(event.score * 100),
				confidencePct: Math.round(event.confidence * 100),
				evidence: {
					event_id: event.eventId,
					primary_detector: event.primaryDetector,
				},
				policyVersion: POLICY_VERSION,
			});
		}
	}

	async listIncidents(limit: number): Promise<unknown[]> {
		return listTier2Incidents(limit);
	}

	async getIncident(incidentId: string): Promise<unknown> {
		const incident = await getTier2IncidentById(incidentId);
		if (!incident) {
			return null;
		}
		const decisions = await listTier2Decisions(incidentId, 50);
		return {
			incident,
			decisions,
		};
	}
}
