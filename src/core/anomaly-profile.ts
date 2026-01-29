import { CUSUM, EWMA, HyperLogLog } from "../algorithms";
import type { settings } from "../config/settings";

export interface AnomalySignal {
	rhythmHash: string;
	service: string;
	severity: string;
	anomalyType: "novelty" | "frequency" | "drift";
	confidence: number;
	context: string;
	timestamp: number;
	metadata: Record<string, unknown>;
}

export class AnomalyProfile {
	readonly rhythmHash: string;
	readonly service: string;
	private frequencyEWMA: EWMA;
	private cardinalityHLL: HyperLogLog;
	private driftCUSUM: CUSUM;
	private lastUpdated: number = 0;
	private eventCount: number = 0;
	private isBaselineEstablished: boolean = false;

	constructor(
		rhythmHash: string,
		service: string,
		config: typeof settings.tier1,
	) {
		this.rhythmHash = rhythmHash;
		this.service = service;
		this.frequencyEWMA = new EWMA({ halfLife: config.ewmaHalfLife });
		this.cardinalityHLL = new HyperLogLog(config.hllPrecision);
		this.driftCUSUM = new CUSUM(config.cusum);
	}

	processEvent(timestamp: number, uniqueId: string): AnomalySignal | null {
		this.eventCount++;
		this.lastUpdated = timestamp;
		this.cardinalityHLL.add(uniqueId);
		const currentFrequency = this.calculateCurrentFrequency();
		this.frequencyEWMA.update(currentFrequency);
		const driftDetected = this.driftCUSUM.update(currentFrequency);

		if (!this.isBaselineEstablished) {
			if (this.eventCount >= 10) {
				this.isBaselineEstablished = true;
			}
			return null;
		}

		const expectedFreq = this.frequencyEWMA.getValue() || 0;
		const stdDev = this.frequencyEWMA.getStdDev();
		// Use 3-sigma (or 2.5 as before) for threshold.
		// Fallback to a small value if stdDev is 0 to avoid over-sensitivity on flatlines.
		const threshold = expectedFreq + 2.5 * Math.max(stdDev, 0.1);

		if (currentFrequency > threshold && this.eventCount >= 3) {
			return {
				rhythmHash: this.rhythmHash,
				service: this.service,
				severity: "WARN",
				anomalyType: "frequency",
				confidence: Math.min(0.95, currentFrequency / threshold - 1),
				context: `Frequency ${currentFrequency.toFixed(1)} exceeds threshold ${threshold.toFixed(1)}`,
				timestamp,
				metadata: {
					expected: expectedFreq,
					actual: currentFrequency,
					stdDev,
				},
			};
		}

		if (driftDetected) {
			return {
				rhythmHash: this.rhythmHash,
				service: this.service,
				severity: "WARN",
				anomalyType: "drift",
				confidence: 0.85,
				context: `CUSUM drift detected: ${this.driftCUSUM.alarmType}`,
				timestamp,
				metadata: {
					cusumValues: this.driftCUSUM.getValues(),
				},
			};
		}

		return null;
	}

	private calculateCurrentFrequency(): number {
		const now = Date.now() / 1000;
		const timeWindow = Math.max(1, now - this.lastUpdated);
		return this.eventCount / (timeWindow / 60);
	}

	getCardinality(): number {
		return this.cardinalityHLL.count();
	}

	getStats() {
		return {
			rhythmHash: this.rhythmHash,
			eventCount: this.eventCount,
			cardinality: this.getCardinality(),
			frequency: this.frequencyEWMA.getValue(),
			lastUpdated: this.lastUpdated,
			isBaselineEstablished: this.isBaselineEstablished,
		};
	}
}
