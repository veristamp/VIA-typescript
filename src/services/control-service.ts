import {
	deletePatch,
	getActivePatches,
	getAllRules,
	patchAnomaly,
} from "../db/registry";

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

	constructor() {
		this.loadPatches();
	}

	private async loadPatches(): Promise<void> {
		const patches = await getActivePatches();
		this.patchRegistry = new Set(patches.map((p) => p.rhythmHash));
		console.log(`Loaded ${this.patchRegistry.size} active patches`);
	}

	async suppressAnomaly(
		rhythmHash: string,
		durationSec: number,
	): Promise<void> {
		const expiryTs = Math.floor(Date.now() / 1000) + durationSec;
		this.suppressionCache.set(rhythmHash, expiryTs);
		console.log(
			`Suppressed rhythm_hash '${rhythmHash}' for ${durationSec} seconds`,
		);
	}

	async patchAnomaly(
		rhythmHash: string,
		reason: string,
		_contextLogs: string[],
	): Promise<void> {
		await patchAnomaly(rhythmHash, reason);
		this.patchRegistry.add(rhythmHash);
		console.log(`Patched rhythm_hash '${rhythmHash}' as ALLOW_LIST`);
	}

	async deletePatch(rhythmHash: string): Promise<void> {
		await deletePatch(rhythmHash);
		this.patchRegistry.delete(rhythmHash);
		console.log(`Deactivated patch for rhythm_hash '${rhythmHash}'`);
	}

	async deleteSuppression(rhythmHash: string): Promise<void> {
		this.suppressionCache.delete(rhythmHash);
		console.log(`Removed suppression for rhythm_hash '${rhythmHash}'`);
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
		const patches = await getAllRules();
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
}
