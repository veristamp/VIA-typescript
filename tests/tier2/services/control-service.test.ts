import { describe, expect, it, mock } from "bun:test";

const activations: string[] = [];
const artifacts: any[] = [];

mock.module("../../../src/db/registry", () => ({
	getActivePatches: async () => [],
	patchAnomaly: async () => {},
	deletePatch: async () => {},
	getAllRules: async () => [],
	listTier2Incidents: async () => [],
	upsertTier1PolicyArtifact: async (input: any) => {
		artifacts.push(input);
	},
	activateTier1Policy: async (policyVersion: string) => {
		activations.push(policyVersion);
	},
	getCurrentActivePolicy: async () => undefined,
	getTier1PolicyByVersion: async (policyVersion: string) => {
		if (policyVersion === "known") {
			return {
				id: 1,
				policyVersion: "known",
				status: "active",
				compiledJson: { version: "known", rules: [] },
				featureFlags: {},
				createdAt: new Date(),
			};
		}
		return undefined;
	},
	listTier1Policies: async () => [],
}));

import { ControlService } from "../../../src/services/control-service";
import type { CompiledPolicyArtifact } from "../../../src/services/policy-compiler-service";

const createMockTier1Sync = () => ({
	isEnabled: () => false,
	sendFeedback: async () => {},
	pushPolicySnapshot: async () => {},
} as any);

describe("ControlService", () => {
	it("compiles policy artifacts through injected compiler", async () => {
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
		const service = new ControlService(compiler as any, createMockTier1Sync());
		const artifact = await service.compilePolicy(10);
		expect(artifact.policyVersion).toBe("policy-1");
		expect(artifacts.length).toBeGreaterThan(0);
	});

	it("publishes known policy versions", async () => {
		const service = new ControlService(undefined, createMockTier1Sync());
		await service.publishPolicy("known");
		expect(activations).toContain("known");
	});

	it("rejects publishing unknown policy versions", async () => {
		const service = new ControlService(undefined, createMockTier1Sync());
		await expect(service.publishPolicy("missing")).rejects.toThrow(
			"policy not found: missing",
		);
	});
});
