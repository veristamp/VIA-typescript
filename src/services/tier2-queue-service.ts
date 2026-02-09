import { settings } from "../config/settings";
import { saveDeadLetter } from "../db/registry";
import { logger } from "../utils/logger";
import type { IncomingAnomalySignal, Tier2Service } from "./tier2-service";

interface QueueTask {
	eventId: string;
	signals: IncomingAnomalySignal[];
	attempts: number;
	enqueuedAt: number;
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
		});
		this.dedupeMap.set(eventId, now + this.dedupeWindowSec);
		this.stats.queued += 1;
		return { accepted: true, eventId };
	}

	private async flush(): Promise<void> {
		if (this.queue.length === 0 || this.inFlight > 0) {
			return;
		}

		const tasks = this.queue.splice(0, this.batchSize);
		if (tasks.length === 0) {
			return;
		}

		this.inFlight += 1;
		this.stats.inFlight = this.inFlight;

		try {
			for (const task of tasks) {
				try {
					await this.tier2Service.processAnomalyBatch(task.signals);
					this.stats.processed += 1;
				} catch (error) {
					task.attempts += 1;
					if (task.attempts < this.maxAttempts) {
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
			}
		} finally {
			this.inFlight -= 1;
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
