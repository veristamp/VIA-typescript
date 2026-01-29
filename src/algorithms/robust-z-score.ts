export class RobustZScore {
	private window: number[];
	private maxSize: number;

	constructor(windowSize: number = 100) {
		this.window = [];
		this.maxSize = windowSize;
	}

	update(sample: number): number {
		this.window.push(sample);
		if (this.window.length > this.maxSize) {
			this.window.shift();
		}

		if (this.window.length < 5) {
			return 0;
		}

		return this.calculateScore(sample);
	}

	private calculateScore(sample: number): number {
		const median = this.getMedian(this.window);
		const mad = this.getMAD(this.window, median);

		if (mad === 0) return 0;

		// 0.6745 is the consistency constant for normal distribution
		return (0.6745 * (sample - median)) / mad;
	}

	private getMedian(values: number[]): number {
		if (values.length === 0) return 0;
		const sorted = [...values].sort((a, b) => a - b);
		const mid = Math.floor(sorted.length / 2);
		return sorted.length % 2 !== 0
			? sorted[mid]
			: (sorted[mid - 1] + sorted[mid]) / 2;
	}

	private getMAD(values: number[], median: number): number {
		const deviations = values.map((v) => Math.abs(v - median));
		return this.getMedian(deviations);
	}
}
