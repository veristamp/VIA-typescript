import type { IncidentCandidate, IncidentStatus } from "../../../types";

export interface ResolvedIncidentDecision {
	status: IncidentStatus;
	confidence: number;
}

export function resolveIncidentDecision(
	candidate: IncidentCandidate,
): ResolvedIncidentDecision {
	const confidence = Math.max(0, Math.min(1, candidate.confidence));
	const status: IncidentStatus =
		candidate.severityMax >= 0.9 || candidate.scoreMax >= 0.95
			? "escalated"
			: candidate.memberPointIds.length >= 3 && confidence >= 0.8
				? "merged"
				: "new";

	return { status, confidence };
}
