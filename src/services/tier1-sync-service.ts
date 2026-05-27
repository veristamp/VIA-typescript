import type { IncidentStatus, Tier1PolicySnapshot } from "../types";
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

export interface Tier1IncidentDecisionRequest {
	severity_max: number;
	score_max: number;
	member_count: number;
	confidence: number;
}

export interface Tier1IncidentDecisionResponse {
	status: IncidentStatus;
	confidence: number;
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

	private requireBaseUrl(): string {
		if (!this.baseUrl) {
			throw new Error("TIER1_BASE_URL is required for Rust Tier-1 sync");
		}
		return this.baseUrl;
	}

	private async postJson<T>(
		path: string,
		body: Record<string, unknown>,
	): Promise<T> {
		const baseUrl = this.requireBaseUrl();
		const response = await fetch(`${baseUrl}${path}`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(body),
		});
		if (!response.ok) {
			const text = await response.text().catch(() => "");
			throw new Error(`Tier1 ${path} failed: ${response.status} ${text}`);
		}
		if (response.status === 204) {
			return undefined as T;
		}
		return (await response.json().catch(() => undefined)) as T;
	}

	private async post(
		path: string,
		body: Record<string, unknown>,
	): Promise<void> {
		await this.postJson<void>(path, body);
	}

	async sendFeedback(events: Tier1FeedbackEvent[]): Promise<void> {
		if (!this.baseUrl || events.length === 0) {
			return;
		}
		for (const event of events) {
			try {
				await this.post(
					"/feedback",
					event as unknown as Record<string, unknown>,
				);
			} catch (error) {
				logger.warn("Failed to send feedback to Tier1", {
					error: String(error),
					pattern_id: event.pattern_id,
				});
			}
		}
	}

	async pushPolicySnapshot(snapshot: Tier1PolicySnapshot): Promise<void> {
		await this.post(
			"/policy/snapshot",
			snapshot as unknown as Record<string, unknown>,
		);
	}

	async resolveIncidentDecision(
		request: Tier1IncidentDecisionRequest,
	): Promise<Tier1IncidentDecisionResponse> {
		const decision = await this.postJson<Tier1IncidentDecisionResponse>(
			"/incident/decision",
			request as unknown as Record<string, unknown>,
		);
		if (
			!decision ||
			!["new", "merged", "escalated"].includes(decision.status) ||
			!Number.isFinite(decision.confidence)
		) {
			throw new Error("Tier1 /incident/decision returned an invalid decision");
		}
		return decision;
	}
}
