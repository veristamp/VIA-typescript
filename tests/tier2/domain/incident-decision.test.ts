import { describe, expect, it } from "bun:test";
import { resolveIncidentDecision } from "../../../src/modules/tier2/domain/incident-decision";
import type { IncidentCandidate } from "../../../src/types";

function candidate(overrides: Partial<IncidentCandidate>): IncidentCandidate {
	return {
		incidentId: "inc-1",
		memberPointIds: ["a"],
		reason: "temporal",
		confidence: 0.7,
		firstSeenTs: 1,
		lastSeenTs: 2,
		severityMax: 0.4,
		scoreMax: 0.5,
		entityKey: "hash:1",
		evidence: {},
		...overrides,
	};
}

describe("resolveIncidentDecision", () => {
	it("escalates when severity or score is very high", () => {
		expect(resolveIncidentDecision(candidate({ severityMax: 0.91 })).status).toBe(
			"escalated",
		);
		expect(resolveIncidentDecision(candidate({ scoreMax: 0.96 })).status).toBe(
			"escalated",
		);
	});

	it("merges when cluster is large and confidence is high", () => {
		const decision = resolveIncidentDecision(
			candidate({ memberPointIds: ["a", "b", "c"], confidence: 0.85 }),
		);
		expect(decision.status).toBe("merged");
	});

	it("falls back to new for lower-signal candidates", () => {
		const decision = resolveIncidentDecision(candidate({}));
		expect(decision.status).toBe("new");
		expect(decision.confidence).toBeCloseTo(0.7, 8);
	});
});
