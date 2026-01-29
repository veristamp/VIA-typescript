export interface CUSUMOptions {
	target: number;
	slack: number;
	threshold: number;
}

export class CUSUM {
	private target: number;
	private slack: number;
	private threshold: number;
	private cPos: number = 0;
	private cNeg: number = 0;
	public alarm: boolean = false;
	public alarmType: "high" | "low" | null = null;
	public alarmValue: number | null = null;

	constructor(options: CUSUMOptions) {
		this.target = options.target;
		this.slack = options.slack;
		this.threshold = options.threshold;
	}

	update(sample: number): boolean {
		this.alarm = false;
		this.alarmType = null;
		this.alarmValue = null;
		
		const deviation = sample - this.target;
		
		// Upper CUSUM: Accumulate positive deviations above slack
		this.cPos = Math.max(0, this.cPos + deviation - this.slack);
		
		// Lower CUSUM: Accumulate negative deviations (deviation is negative, so -deviation is positive)
		this.cNeg = Math.max(0, this.cNeg - deviation - this.slack);

		if (this.cPos > this.threshold) {
			this.alarm = true;
			this.alarmType = "high";
			this.alarmValue = this.cPos;
			// Optional: Reset partially or fully after alarm. 
			// Standard CUSUM resets to 0.
			this.cPos = 0; 
			return true;
		}

		if (this.cNeg > this.threshold) {
			this.alarm = true;
			this.alarmType = "low";
			this.alarmValue = this.cNeg;
			this.cNeg = 0;
			return true;
		}

		return false;
	}

	setTarget(target: number): void {
		this.target = target;
	}

	setParameters(target: number, slack: number, threshold: number): void {
		this.target = target;
		this.slack = slack;
		this.threshold = threshold;
	}

	getValues(): { upper: number; lower: number } {
		return { upper: this.cPos, lower: this.cNeg };
	}

	reset(): void {
		this.cPos = 0;
		this.cNeg = 0;
		this.alarm = false;
		this.alarmType = null;
		this.alarmValue = null;
	}
}
