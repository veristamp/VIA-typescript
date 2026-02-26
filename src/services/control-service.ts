import { tier2ControlRepository } from "../modules/tier2/adapters/registry-repositories";
import type { Tier2ControlRepository } from "../modules/tier2/ports/repositories";
import type { Tier1PolicySnapshot } from "../types";
import { logger } from "../utils/logger";
import type {
	CompiledPolicyArtifact,
	PolicyCompilerService,
} from "./policy-compiler-service";

export interface SuppressRequest {
	rhythmHash: string;
	durationSec: number;
}

export interface PatchRequest {
	rhythmHash: string;
	reason: string;
	contextLogs: string[];
}

export class ControlService {
	private suppressionCache: Map<string, number> = new Map();
	private patchRegistry: Set<string> = new Set();
	private ready: Promise<void> = Promise.resolve();
	private initialized = false;

	constructor(
		private readonly policyCompiler?: PolicyCompilerService,
		private readonly repository: Tier2ControlRepository = tier2ControlRepository,
	) {
		// Tables are initialized during application bootstrap, so defer DB reads.
	}

	async initialize(): Promise<void> {
		if (this.initialized) {
			return;
		}
		this.ready = this.loadPatches();
		await this.ready;
		this.initialized = true;
	}

	private async loadPatches(): Promise<void> {
		const patches = await this.repository.getActivePatches();
		this.patchRegistry = new Set(patches.map((p) => p.rhythmHash));
		logger.info("Loaded active control patches", {
			count: this.patchRegistry.size,
		});
	}

	async suppressAnomaly(
		rhythmHash: string,
		durationSec: number,
	): Promise<void> {
		const expiryTs = Math.floor(Date.now() / 1000) + durationSec;
		this.suppressionCache.set(rhythmHash, expiryTs);
		logger.info("Suppressed anomaly", { rhythmHash, durationSec, expiryTs });
	}

	async patchAnomaly(
		rhythmHash: string,
		reason: string,
		_contextLogs: string[],
	): Promise<void> {
		await this.repository.patchAnomaly(rhythmHash, reason);
		this.patchRegistry.add(rhythmHash);
		logger.info("Patched anomaly", { rhythmHash, reason });
	}

	async deletePatch(rhythmHash: string): Promise<void> {
		await this.repository.deletePatch(rhythmHash);
		this.patchRegistry.delete(rhythmHash);
		logger.info("Deleted patch", { rhythmHash });
	}

	async deleteSuppression(rhythmHash: string): Promise<void> {
		this.suppressionCache.delete(rhythmHash);
		logger.info("Deleted suppression", { rhythmHash });
	}

	isSuppressedOrPatched(rhythmHash: string): boolean {
		// Check if permanently patched
		if (this.patchRegistry.has(rhythmHash)) {
			return true;
		}

		// Check if temporarily suppressed
		const expiryTs = this.suppressionCache.get(rhythmHash);
		if (expiryTs && Date.now() / 1000 < expiryTs) {
			return true;
		}

		return false;
	}

	async getAllRules() {
		const patches = await this.repository.getAllRules();
		const suppressions: Array<{ rhythmHash: string; expiresAt: number }> = [];

		// Get temporary suppressions from cache
		const now = Math.floor(Date.now() / 1000);
		for (const [hash, expiryTs] of this.suppressionCache.entries()) {
			if (expiryTs > now) {
				suppressions.push({ rhythmHash: hash, expiresAt: expiryTs });
			}
		}

		return {
			patches,
			suppressions,
		};
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

		logger.info("Compiled Tier-1 policy artifact", {
			policyVersion: artifact.policyVersion,
			ruleCount: artifact.snapshot.rules.length,
		});

		return artifact;
	}

	async publishPolicy(policyVersion: string): Promise<void> {
		const policy = await this.repository.getTier1PolicyByVersion(policyVersion);
		if (!policy) {
			throw new Error(`policy not found: ${policyVersion}`);
		}
		await this.repository.activateTier1Policy(policyVersion);
		logger.info("Published Tier-1 policy", { policyVersion });
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

		logger.warn("Rolled back Tier-1 policy", {
			fromVersion: targetVersion,
			toVersion: rollbackVersion,
			reason,
		});
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
