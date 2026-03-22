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
		candidate.severityMax >= 0.5 || candidate.scoreMax >= 0.6
			? "escalated"
			: candidate.memberPointIds.length >= 2 && confidence >= 0.3
				? "merged"
				: "new";

	return { status, confidence };
}
