import type { Tier1Engine } from "../services/tier1-engine";
import type { BatchSummary, LogRecord } from "../types";
import { AsyncQueue } from "./queue";

export class QueueWorker {
	private queue: AsyncQueue<LogRecord>;
	private engine: Tier1Engine;
	private isRunning: boolean = false;
	private batchSize: number;
	private flushInterval: number;

	constructor(
		engine: Tier1Engine,
		options: { batchSize: number; flushInterval: number },
	) {
		this.queue = new AsyncQueue(10000);
		this.engine = engine;
		this.batchSize = options.batchSize || 100;
		this.flushInterval = options.flushInterval || 1000;
	}

	async enqueue(log: LogRecord): Promise<void> {
		await this.queue.put(log);
	}

	start(): void {
		if (this.isRunning) return;
		this.isRunning = true;

		// Start batch processor
		this.runBatchLoop();

		// Start periodic flush
		setInterval(() => this.flush(), this.flushInterval);
	}

	stop(): void {
		this.isRunning = false;
	}

	private async runBatchLoop(): Promise<void> {
		const batch: LogRecord[] = [];

		while (this.isRunning) {
			try {
				const log = await this.queue.get();
				if (!log) break;

				batch.push(log);

				if (batch.length >= this.batchSize) {
					await this.processBatch(batch);
					batch.length = 0;
				}
			} catch (error) {
				console.error("Worker error:", error);
			}
		}
	}

	private async processBatch(batch: LogRecord[]): Promise<void> {
		const summary = this.createBatchSummary(batch);
		await this.engine.processSummary(summary);
	}

	private createBatchSummary(batch: LogRecord[]): BatchSummary {
		// Group by rhythm_hash
		const groups = new Map<
			string,
			{
				service: string;
				count: number;
				uniqueIds: string[];
			}
		>();

		for (const log of batch) {
			const hash = log.rhythmHash;
			if (!groups.has(hash)) {
				groups.set(hash, {
					service: log.service,
					count: 0,
					uniqueIds: [],
				});
			}

			const group = groups.get(hash);
			if (group) {
				group.count++;
				group.uniqueIds.push(log.id);
			}
		}

		return {
			timestamp: Date.now() / 1000,
			totalLogs: batch.length,
			groups: Array.from(groups.values()).map((group) => ({
				rhythmHash: group.service,
				service: group.service,
				count: group.count,
				uniqueIds: group.uniqueIds,
			})),
		};
	}

	private async flush(): Promise<void> {
		// Process any remaining items
		while (!this.queue.isEmpty) {
			const batch: LogRecord[] = [];
			while (batch.length < this.batchSize && !this.queue.isEmpty) {
				const log = await this.queue.get();
				if (!log) break;
				batch.push(log);
			}

			if (batch.length > 0) {
				await this.processBatch(batch);
			}
		}
	}
}
