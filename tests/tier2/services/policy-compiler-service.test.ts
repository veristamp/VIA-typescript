import { describe, expect, it } from "bun:test";
import type { Tier2Incident } from "../../../src/db/schema";
import { PolicyCompilerService } from "../../../src/services/policy-compiler-service";

function incident(overrides: Partial<Tier2Incident>): Tier2Incident {
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
	it("builds boost rules from high-confidence Rust incident decisions", () => {
		const compiler = new PolicyCompilerService();
		const artifacts = compiler.compile([
			incident({ incidentId: "inc-e", status: "escalated" }),
			incident({ incidentId: "inc-m", status: "merged" }),
		]);

		expect(artifacts.snapshot.rules.length).toBe(2);
		expect(
			artifacts.snapshot.rules.some((r) => r.action === "boost"),
		).toBeTrue();
	});

	it("drops low-confidence incidents from policy output", () => {
		const compiler = new PolicyCompilerService();
		const artifacts = compiler.compile([
			incident({ incidentId: "inc-low", confidence: 40, status: "escalated" }),
		]);
		expect(artifacts.snapshot.rules.length).toBe(0);
	});

	it("only emits entity hashes that are safe numeric values for Tier-1", () => {
		const compiler = new PolicyCompilerService();
		const artifacts = compiler.compile([
			incident({
				incidentId: "inc-safe",
				status: "escalated",
				entityKey: "hash:12345",
			}),
			incident({
				incidentId: "inc-unsafe",
				status: "merged",
				entityKey: "hash:18446744073709551615",
			}),
		]);
		const safeRule = artifacts.snapshot.rules.find(
			(rule) => rule.pattern_id === "inc-safe",
		);
		const unsafeRule = artifacts.snapshot.rules.find(
			(rule) => rule.pattern_id === "inc-unsafe",
		);
		expect(safeRule?.entity_hashes).toEqual([12345]);
		expect(unsafeRule?.entity_hashes ?? []).toEqual([]);
	});
});
