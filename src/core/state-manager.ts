import type { settings } from "../config/settings";
import { AnomalyProfile } from "./anomaly-profile";

export class StateManager {
	private profiles: Map<string, AnomalyProfile> = new Map();
	private config: typeof settings;

	constructor(config: typeof settings) {
		this.config = config;
	}

	getOrCreateProfile(rhythmHash: string, service: string): AnomalyProfile {
		let profile = this.profiles.get(rhythmHash);
		if (!profile) {
			profile = new AnomalyProfile(rhythmHash, service, this.config.tier1);
			this.profiles.set(rhythmHash, profile);
		}
		return profile;
	}

	getProfile(rhythmHash: string): AnomalyProfile | undefined {
		return this.profiles.get(rhythmHash);
	}

	getAllProfiles(): AnomalyProfile[] {
		return Array.from(this.profiles.values());
	}

	getStats() {
		return {
			totalProfiles: this.profiles.size,
			memoryEstimateMB: this.estimateMemoryUsage(),
		};
	}

	private estimateMemoryUsage(): number {
		return (this.profiles.size * 15) / 1024;
	}

	cleanup(maxAgeSeconds: number): number {
		const now = Date.now() / 1000;
		let removed = 0;

		for (const [hash, profile] of this.profiles) {
			const lastUpdated = (profile as unknown as { lastUpdated: number })
				.lastUpdated;
			if (now - lastUpdated > maxAgeSeconds) {
				this.profiles.delete(hash);
				removed++;
			}
		}

		return removed;
	}
}
