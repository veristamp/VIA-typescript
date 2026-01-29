export interface QueueItem<T> {
	id: string;
	timestamp: number;
	data: T;
}

export class AsyncQueue<T> {
	private queue: T[] = [];
	private resolvers: ((value: T) => void)[] = [];
	private maxSize: number;

	constructor(maxSize: number = 10000) {
		this.maxSize = maxSize;
	}

	async put(item: T): Promise<void> {
		if (this.resolvers.length > 0) {
			const resolve = this.resolvers.shift();
			if (resolve) {
				resolve(item);
			}
			return;
		}

		if (this.queue.length >= this.maxSize) {
			throw new Error("Queue full - backpressure detected");
		}

		this.queue.push(item);
	}

	async get(): Promise<T> {
		if (this.queue.length > 0) {
			const item = this.queue.shift();
			if (item !== undefined) {
				return Promise.resolve(item);
			}
		}

		return new Promise((resolve) => {
			this.resolvers.push(resolve);
		});
	}

	get size(): number {
		return this.queue.length;
	}

	get isEmpty(): boolean {
		return this.queue.length === 0;
	}
}
