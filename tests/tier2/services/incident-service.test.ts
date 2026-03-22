import { describe, expect, it, mock } from "bun:test";

const upserts: any[] = [];
const decisions: any[] = [];

mock.module("../../../src/db/registry", () => ({
	upsertTier2Incident: async (input: any) => {
		upserts.push(input);
	},
	saveTier2Decision: async (...args: any[]) => {
		decisions.push(args);
	},
	getTier2IncidentById: async (incidentId: string) => {
		if (incidentId === "inc-found") {
			return {
				id: 1,
				incidentId,
				status: "new",
				entityKey: "hash:1",
				firstSeenTs: 1,
				lastSeenTs: 1,
				severityMax: 40,
				scoreMax: 50,
				confidence: 80,
				evidence: {},
				policyVersion: "v1",
				updatedAt: new Date(),
				createdAt: new Date(),
			};
		}
		return undefined;
	},
	listTier2Incidents: async () => [],
	listTier2IncidentsForRun: async () => [],
	listTier2Decisions: async () => [],
	getLatestDeadLetters: async () => [],
}));

import { IncidentService } from "../../../src/services/incident-service";
import type { IncidentCandidate } from "../../../src/types";

function candidate(overrides: Partial<IncidentCandidate>): IncidentCandidate {
	return {
		incidentId: "inc-1",
		memberPointIds: ["p1"],
		reason: "temporal",
		confidence: 0.8,
		firstSeenTs: 1,
		lastSeenTs: 2,
		severityMax: 0.4,
		scoreMax: 0.5,
		entityKey: "hash:1",
		evidence: {},
		...overrides,
	};
}

const createMockTier1Sync = () => ({
	isEnabled: () => false,
	sendFeedback: async () => {},
	pushPolicySnapshot: async () => {},
} as any);

describe("IncidentService", () => {
	it("persists incident decisions with normalized percentage values", async () => {
		const service = new IncidentService(createMockTier1Sync());

		await service.applyCandidates([
			candidate({ severityMax: 0.95, scoreMax: 0.6, confidence: 0.92 }),
		]);

		expect(upserts.length).toBeGreaterThan(0);
		const saved = upserts[upserts.length - 1];
		expect(saved.status).toBe("escalated");
		expect(saved.severityMaxPct).toBe(95);
		expect(decisions.length).toBeGreaterThan(0);
	});

	it("returns null for missing incident lookup", async () => {
		const service = new IncidentService(createMockTier1Sync());
		const incident = await service.getIncident("missing");
		expect(incident).toBeNull();
	});
});
