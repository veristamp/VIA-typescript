export class EWMA {
	private alpha: number;
	private mean: number = 0;
	private variance: number = 0;
	private initialized: boolean = false;

	constructor(options: { halfLife: number } | { alpha: number }) {
		if ("halfLife" in options) {
			this.alpha = 1 - Math.exp(-Math.LN2 / options.halfLife);
		} else {
			this.alpha = options.alpha;
		}
	}

	update(sample: number): number {
		if (!this.initialized) {
			this.mean = sample;
			this.variance = 0;
			this.initialized = true;
		} else {
			const diff = sample - this.mean;
			const incr = this.alpha * diff;
			this.mean += incr;
			
			// EWMVar update: (1 - alpha) * (Var_prev + alpha * diff^2)
			// Or approximate incremental: Var_new = (1-alpha)*Var_old + alpha * (diff * (sample - new_mean))
			// Standard EWMVar:
			this.variance = (1 - this.alpha) * (this.variance + this.alpha * diff * diff);
		}
		return this.mean;
	}

	getValue(): number {
		return this.mean;
	}

	getStdDev(): number {
		return Math.sqrt(this.variance);
	}

	/**
	 * Returns the number of standard deviations the sample is from the mean
	 */
	getZScore(sample: number): number {
		const std = this.getStdDev();
		if (std === 0) return 0;
		return (sample - this.mean) / std;
	}

	reset(): void {
		this.mean = 0;
		this.variance = 0;
		this.initialized = false;
	}
}
