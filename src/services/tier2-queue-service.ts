import { settings } from "../config/settings";
import { saveDeadLetter } from "../db/registry";
import { logger } from "../utils/logger";
import type { IncomingAnomalySignal, Tier2Service } from "./tier2-service";

interface QueueTask {
	eventId: string;
	signals: IncomingAnomalySignal[];
	attempts: number;
	enqueuedAt: number;
	nextAttemptAt: number;
	priority: "critical" | "normal";
}

export interface QueueStats {
	queued: number;
	processed: number;
	dropped: number;
	retried: number;
	dlq: number;
	inFlight: number;
}

export class Tier2QueueService {
	private queue: QueueTask[] = [];
	private readonly dedupeWindowSec = 900;
	private readonly dedupeMap = new Map<string, number>();
	private readonly maxSize = settings.queue.maxSize;
	private readonly batchSize = settings.queue.batchSize;
	private readonly maxAttempts = 3;
	private flushTimer: Timer | null = null;
	private readonly maxWorkers = settings.queue.maxWorkers;
	private inFlight = 0;
	private stats: QueueStats = {
		queued: 0,
		processed: 0,
		dropped: 0,
		retried: 0,
		dlq: 0,
		inFlight: 0,
	};

	constructor(private readonly tier2Service: Tier2Service) {}

	start(): void {
		if (this.flushTimer) {
			return;
		}
		this.flushTimer = setInterval(() => {
			void this.flush();
		}, settings.queue.flushInterval);
	}

	stop(): void {
		if (this.flushTimer) {
			clearInterval(this.flushTimer);
			this.flushTimer = null;
		}
	}

	private cleanupDedupe(): void {
		const now = Math.floor(Date.now() / 1000);
		for (const [eventId, expiry] of this.dedupeMap.entries()) {
			if (expiry <= now) {
				this.dedupeMap.delete(eventId);
			}
		}
	}

	enqueue(signals: IncomingAnomalySignal[]): {
		accepted: boolean;
		eventId: string;
		reason?: string;
	} {
		const eventId = this.tier2Service.deriveBatchEventId(signals);
		const now = Math.floor(Date.now() / 1000);
		this.cleanupDedupe();

		if (this.dedupeMap.has(eventId)) {
			this.stats.dropped += 1;
			return { accepted: false, eventId, reason: "duplicate_batch" };
		}

		if (this.queue.length >= this.maxSize) {
			this.stats.dropped += 1;
			void saveDeadLetter(eventId, "queue_full", {
				signals_count: signals.length,
			});
			return { accepted: false, eventId, reason: "queue_full" };
		}

		this.queue.push({
			eventId,
			signals,
			attempts: 0,
			enqueuedAt: now,
			nextAttemptAt: now,
			priority: this.resolvePriority(signals),
		});
		this.dedupeMap.set(eventId, now + this.dedupeWindowSec);
		this.stats.queued += 1;
		return { accepted: true, eventId };
	}

	private resolvePriority(
		signals: IncomingAnomalySignal[],
	): "critical" | "normal" {
		const maxSeverity = signals.reduce(
			(acc, signal) => Math.max(acc, signal.severity),
			0,
		);
		return maxSeverity >= 0.85 ? "critical" : "normal";
	}

	private pickRunnableTasks(now: number): QueueTask[] {
		if (this.queue.length === 0) {
			return [];
		}

		const runnable: QueueTask[] = [];
		const kept: QueueTask[] = [];

		for (const task of this.queue) {
			if (task.nextAttemptAt <= now) {
				runnable.push(task);
			} else {
				kept.push(task);
			}
		}

		runnable.sort((a, b) => {
			const pa = a.priority === "critical" ? 0 : 1;
			const pb = b.priority === "critical" ? 0 : 1;
			if (pa !== pb) {
				return pa - pb;
			}
			return a.enqueuedAt - b.enqueuedAt;
		});

		const selected = runnable.slice(0, this.batchSize);
		const deferred = runnable.slice(this.batchSize);
		this.queue = [...kept, ...deferred];
		return selected;
	}

	private async flush(): Promise<void> {
		if (this.queue.length === 0 || this.inFlight >= this.maxWorkers) {
			return;
		}

		const now = Math.floor(Date.now() / 1000);
		const capacity = this.maxWorkers - this.inFlight;
		const tasks = this.pickRunnableTasks(now).slice(0, capacity);
		if (tasks.length === 0) {
			return;
		}

		this.inFlight += tasks.length;
		this.stats.inFlight = this.inFlight;

		const workers = tasks.map(async (task) => {
			try {
				await this.tier2Service.processAnomalyBatch(task.signals);
				this.stats.processed += 1;
			} catch (error) {
				task.attempts += 1;
				if (task.attempts < this.maxAttempts) {
					const jitterMs = Math.floor(Math.random() * 100);
					const backoffMs =
						settings.queue.retryBaseDelayMs *
							2 ** Math.max(0, task.attempts - 1) +
						jitterMs;
					task.nextAttemptAt = Math.floor((Date.now() + backoffMs) / 1000);
					this.queue.push(task);
					this.stats.retried += 1;
				} else {
					this.stats.dlq += 1;
					await saveDeadLetter(task.eventId, "processing_failed", {
						error: String(error),
						attempts: task.attempts,
						age_sec: Math.max(
							0,
							Math.floor(Date.now() / 1000) - task.enqueuedAt,
						),
					});
					logger.error("Task moved to DLQ", {
						eventId: task.eventId,
						error: String(error),
					});
				}
			}
		});

		try {
			await Promise.all(workers);
		} finally {
			this.inFlight = Math.max(0, this.inFlight - tasks.length);
			this.stats.inFlight = this.inFlight;
		}
	}

	getStats(): QueueStats {
		return {
			...this.stats,
			queued: this.queue.length,
			inFlight: this.inFlight,
		};
	}
}
