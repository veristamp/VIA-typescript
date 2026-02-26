import { describe, expect, it } from "bun:test";
import type { Tier2ControlRepository } from "../../../src/modules/tier2/ports/repositories";
import { ControlService } from "../../../src/services/control-service";
import type { CompiledPolicyArtifact } from "../../../src/services/policy-compiler-service";

function createControlRepo(): Tier2ControlRepository & {
	activations: string[];
	artifacts: unknown[];
} {
	return {
		activations: [],
		artifacts: [],
		async getActivePatches() {
			return [];
		},
		async patchAnomaly() {},
		async deletePatch() {},
		async getAllRules() {
			return [];
		},
		async listTier2Incidents() {
			return [];
		},
		async upsertTier1PolicyArtifact(input) {
			this.artifacts.push(input);
		},
		async activateTier1Policy(policyVersion) {
			this.activations.push(policyVersion);
		},
		async getCurrentActivePolicy() {
			return undefined;
		},
		async getTier1PolicyByVersion(policyVersion) {
			if (policyVersion === "known") {
				return {
					id: 1,
					policyVersion: "known",
					status: "draft",
					compiledJson: { version: "known", created_at_unix: 1, rules: [], defaults: { score_scale: 1, confidence_scale: 1 } },
					featureFlags: {},
					rollbackOf: null,
					createdAt: new Date(),
				};
			}
			return undefined;
		},
		async listTier1Policies() {
			return [];
		},
	};
}

describe("ControlService", () => {
	it("compiles policy artifacts through injected compiler and repository", async () => {
		const repo = createControlRepo();
		const compiler = {
			compile: () =>
				({
					policyVersion: "policy-1",
					snapshot: {
						version: "policy-1",
						created_at_unix: 1,
						rules: [],
						defaults: { score_scale: 1, confidence_scale: 1 },
					},
					featureFlags: {},
				}) as CompiledPolicyArtifact,
		};
		const service = new ControlService(compiler as never, repo);
		const artifact = await service.compilePolicy(10);
		expect(artifact.policyVersion).toBe("policy-1");
		expect(repo.artifacts.length).toBe(1);
	});

	it("publishes known policy versions", async () => {
		const repo = createControlRepo();
		const service = new ControlService(undefined, repo);
		await service.publishPolicy("known");
		expect(repo.activations).toEqual(["known"]);
	});

	it("rejects publishing unknown policy versions", async () => {
		const repo = createControlRepo();
		const service = new ControlService(undefined, repo);
		await expect(service.publishPolicy("missing")).rejects.toThrow(
			"policy not found: missing",
		);
	});
});
