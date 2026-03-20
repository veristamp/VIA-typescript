import { describe, expect, it } from "bun:test";
import type {
	Tier2DeadLetterRepository,
	Tier2IncidentRepository,
} from "../../../src/modules/tier2/ports/repositories";
import { IncidentService } from "../../../src/services/incident-service";
import type { Tier1SyncService } from "../../../src/services/tier1-sync-service";
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

function createRepo(): Tier2IncidentRepository & {
	upserts: unknown[];
	decisions: unknown[];
} {
	return {
		upserts: [],
		decisions: [],
		async upsertIncident(input) {
			this.upserts.push(input);
		},
		async saveDecision(...args) {
			this.decisions.push(args);
		},
		async getIncidentById(incidentId) {
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
		async listIncidents() {
			return [];
		},
		async listIncidentsForRun() {
			return [];
		},
		async listDecisions() {
			return [];
		},
	};
}

function createMockDeadLetterRepo(): Tier2DeadLetterRepository {
	return {
		async saveDeadLetter() {},
		async getLatestDeadLetters() { return []; },
	};
}

function createMockTier1Sync(): Tier1SyncService {
	return {
		isEnabled: () => false,
		async sendFeedback() {},
		async pushPolicySnapshot() {},
	} as unknown as Tier1SyncService;
}

describe("IncidentService", () => {
	it("persists incident decisions with normalized percentage values", async () => {
		const repo = createRepo();
		const service = new IncidentService(repo, createMockDeadLetterRepo(), createMockTier1Sync());

		await service.applyCandidates([
			candidate({ severityMax: 0.95, scoreMax: 0.6, confidence: 0.92 }),
		]);

		expect(repo.upserts.length).toBe(1);
		const saved = repo.upserts[0] as { status: string; severityMaxPct: number };
		expect(saved.status).toBe("escalated");
		expect(saved.severityMaxPct).toBe(95);
		expect(repo.decisions.length).toBe(1);
	});

	it("returns null for missing incident lookup", async () => {
		const repo = createRepo();
		const service = new IncidentService(repo, createMockDeadLetterRepo(), createMockTier1Sync());
		const incident = await service.getIncident("missing");
		expect(incident).toBeNull();
	});
});
