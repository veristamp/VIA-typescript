import { describe, expect, it } from "bun:test";
import type { Tier2Incident } from "../db/schema";
import { PolicyCompilerService } from "./policy-compiler-service";

function incident(
	overrides: Partial<Tier2Incident>,
): Tier2Incident {
	return {
		id: 1,
		incidentId: "inc-1",
		status: "new",
		entityKey: "hash:123",
		firstSeenTs: 1,
		lastSeenTs: 2,
		severityMax: 95,
		scoreMax: 95,
		confidence: 90,
		evidence: {},
		policyVersion: "v1",
		updatedAt: new Date(),
		createdAt: new Date(),
		...overrides,
	};
}

describe("PolicyCompilerService", () => {
	it("builds suppress and boost rules from high-confidence incidents", () => {
		const compiler = new PolicyCompilerService();
		const artifacts = compiler.compile([
			incident({ incidentId: "inc-s", status: "suppressed" }),
			incident({ incidentId: "inc-e", status: "escalated" }),
			incident({ incidentId: "inc-m", status: "merged" }),
		]);

		expect(artifacts.snapshot.rules.length).toBe(3);
		expect(
			artifacts.snapshot.rules.some((r) => r.action === "suppress"),
		).toBeTrue();
		expect(artifacts.snapshot.rules.some((r) => r.action === "boost")).toBeTrue();
	});

	it("drops low-confidence incidents from policy output", () => {
		const compiler = new PolicyCompilerService();
		const artifacts = compiler.compile([
			incident({ incidentId: "inc-low", confidence: 40, status: "suppressed" }),
		]);
		expect(artifacts.snapshot.rules.length).toBe(0);
	});
});

