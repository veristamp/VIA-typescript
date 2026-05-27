import { tier2ControlRepository } from "../modules/tier2/adapters/registry-repositories";
import type { Tier2ControlRepository } from "../modules/tier2/ports/repositories";
import type { Tier1PolicySnapshot } from "../types";
import { logger } from "../utils/logger";
import type {
	CompiledPolicyArtifact,
	PolicyCompilerService,
} from "./policy-compiler-service";
import { Tier1SyncService } from "./tier1-sync-service";

export class ControlService {
	constructor(
		private readonly policyCompiler?: PolicyCompilerService,
		private readonly repository: Tier2ControlRepository = tier2ControlRepository,
		private readonly tier1Sync: Tier1SyncService = new Tier1SyncService(),
	) {
		// Tables are initialized during application bootstrap, so defer DB reads.
	}

	async compilePolicy(limit: number = 250): Promise<CompiledPolicyArtifact> {
		if (!this.policyCompiler) {
			throw new Error("policy compiler is not configured");
		}
		const incidents = await this.repository.listTier2Incidents(limit);
		const artifact = this.policyCompiler.compile(incidents);

		await this.repository.upsertTier1PolicyArtifact({
			policyVersion: artifact.policyVersion,
			status: "draft",
			compiledJson: artifact.snapshot as unknown as Record<string, unknown>,
			featureFlags: artifact.featureFlags,
		});

		return artifact;
	}

	async publishPolicy(policyVersion: string): Promise<void> {
		const policy = await this.repository.getTier1PolicyByVersion(policyVersion);
		if (!policy) {
			throw new Error(`policy not found: ${policyVersion}`);
		}
		await this.repository.activateTier1Policy(policyVersion);
		try {
			await this.tier1Sync.pushPolicySnapshot(
				policy.compiledJson as Tier1PolicySnapshot,
			);
		} catch (error) {
			logger.error("Failed pushing policy snapshot to Tier1", {
				policyVersion,
				error: String(error),
			});
			throw error;
		}
	}

	async rollbackPolicy(targetVersion: string, reason: string): Promise<string> {
		const target = await this.repository.getTier1PolicyByVersion(targetVersion);
		if (!target) {
			throw new Error(`policy not found: ${targetVersion}`);
		}

		const rollbackVersion = `${targetVersion}-rollback-${Math.floor(Date.now() / 1000)}`;
		await this.repository.upsertTier1PolicyArtifact({
			policyVersion: rollbackVersion,
			status: "active",
			compiledJson: target.compiledJson as Record<string, unknown>,
			featureFlags: {
				...(target.featureFlags as Record<string, unknown>),
				rollback_reason: reason,
			},
			rollbackOf: targetVersion,
		});
		await this.repository.activateTier1Policy(rollbackVersion);

		return rollbackVersion;
	}

	async getCurrentPolicy(): Promise<Tier1PolicySnapshot | null> {
		const active = await this.repository.getCurrentActivePolicy();
		if (!active) {
			return null;
		}
		return active.compiledJson as Tier1PolicySnapshot;
	}

	async getPolicyByVersion(
		policyVersion: string,
	): Promise<Tier1PolicySnapshot | null> {
		const policy = await this.repository.getTier1PolicyByVersion(policyVersion);
		if (!policy) {
			return null;
		}
		return policy.compiledJson as Tier1PolicySnapshot;
	}

	async listPolicies(limit: number = 50) {
		return this.repository.listTier1Policies(limit);
	}
}
