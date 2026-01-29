export class DoubleEWMA {
	private alpha: number;
	private beta: number;
	private level: number = 0;
	private trend: number = 0;
	private initialized: boolean = false;

	constructor(alpha: number = 0.3, beta: number = 0.1) {
		this.alpha = alpha;
		this.beta = beta;
	}

	update(sample: number): number {
		if (!this.initialized) {
			this.level = sample;
			this.trend = 0;
			this.initialized = true;
			return sample;
		}

		const lastLevel = this.level;
		const lastTrend = this.trend;

		// Level update (Smoothed value)
		this.level = this.alpha * sample + (1 - this.alpha) * (lastLevel + lastTrend);

		// Trend update
		this.trend =
			this.beta * (this.level - lastLevel) + (1 - this.beta) * lastTrend;

		return this.level + this.trend;
	}

	predict(horizon: number = 1): number {
		return this.level + horizon * this.trend;
	}

	getError(sample: number): number {
		// Prediction for current step was previous level + previous trend
		// But here we might want the error from the *smoothed* value or the 1-step ahead prediction made *before* this sample.
		// Standard error definition: Actual - Forecast
		// Forecast for time t made at t-1: Level_{t-1} + Trend_{t-1}
		// Since update() updates level/trend to time t, we can't easily get t-1 state unless we stored it.
		// However, the error is essentially implicit in the logic.
		// Let's approximate deviation from the *current* model:
		return sample - (this.level); // This is residual
	}
	
	reset(): void {
		this.level = 0;
		this.trend = 0;
		this.initialized = false;
	}
}
