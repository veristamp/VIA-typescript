import type { Tier1PolicySnapshot } from "../types";
import { logger } from "../utils/logger";

export interface Tier1FeedbackEvent {
	entity_hash_text?: string;
	entity_id?: string;
	signal_timestamp: number;
	was_true_positive: boolean;
	detector_scores: number[];
	source: string;
	confidence: number;
	label_class?: "true_positive" | "false_positive" | "false_negative";
	pattern_id?: string;
	feedback_latency_ms?: number;
}

export class Tier1SyncService {
	private readonly baseUrl: string | null;

	constructor() {
		const configured = process.env.TIER1_BASE_URL?.trim();
		this.baseUrl =
			configured && configured.length > 0
				? configured.replace(/\/$/, "")
				: null;
	}

	isEnabled(): boolean {
		return this.baseUrl !== null;
	}

	private async post(path: string, body: Record<string, unknown>): Promise<void> {
		if (!this.baseUrl) {
			return;
		}
		const response = await fetch(`${this.baseUrl}${path}`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(body),
		});
		if (!response.ok) {
			const text = await response.text().catch(() => "");
			throw new Error(`Tier1 ${path} failed: ${response.status} ${text}`);
		}
	}

	async sendFeedback(events: Tier1FeedbackEvent[]): Promise<void> {
		if (!this.baseUrl || events.length === 0) {
			return;
		}
		for (const event of events) {
			try {
				await this.post("/feedback", event as unknown as Record<string, unknown>);
			} catch (error) {
				logger.warn("Failed to send feedback to Tier1", {
					error: String(error),
					pattern_id: event.pattern_id,
				});
			}
		}
	}

	async pushPolicySnapshot(snapshot: Tier1PolicySnapshot): Promise<void> {
		if (!this.baseUrl) {
			return;
		}
		await this.post(
			"/policy/snapshot",
			snapshot as unknown as Record<string, unknown>,
		);
	}
}

