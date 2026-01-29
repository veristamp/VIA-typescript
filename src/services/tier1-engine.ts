import type { settings } from "../config/settings";
import { type AnomalySignal, StateManager } from "../core";

export interface BatchSummary {
	timestamp: number;
	totalLogs: number;
	groups: Array<{
		rhythmHash: string;
		service: string;
		count: number;
		uniqueIds: string[];
	}>;
}

export class Tier1Engine {
	private stateManager: StateManager;

	constructor(config: typeof settings) {
		this.stateManager = new StateManager(config);
	}

	async processSummary(summary: BatchSummary): Promise<AnomalySignal[]> {
		const signals: AnomalySignal[] = [];

		for (const group of summary.groups) {
			const profile = this.stateManager.getOrCreateProfile(
				group.rhythmHash,
				group.service,
			);

			for (const uniqueId of group.uniqueIds) {
				const signal = profile.processEvent(summary.timestamp, uniqueId);
				if (signal) {
					signals.push(signal);
				}
			}
		}

		return signals;
	}

	getStats() {
		return this.stateManager.getStats();
	}

	getProfile(rhythmHash: string) {
		return this.stateManager.getProfile(rhythmHash);
	}

	getAllProfiles() {
		return this.stateManager.getAllProfiles();
	}
}
