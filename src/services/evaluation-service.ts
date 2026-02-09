import {
	getLatestEvaluationMetrics,
	saveEvaluationMetrics,
} from "../db/registry";
import { logger } from "../utils/logger";

export interface GroundTruth {
	timestamp: number;
	isAnomaly: boolean;
	scenarioName: string;
}

export interface DetectionResult {
	timestamp: number;
	isAnomaly: boolean;
	confidence: number;
}

export class EvaluationService {
	private groundTruthBuffer: GroundTruth[] = [];
	private detectionBuffer: DetectionResult[] = [];

	recordGroundTruth(truth: GroundTruth) {
		this.groundTruthBuffer.push(truth);
		// Keep buffer size manageable
		if (this.groundTruthBuffer.length > 1000) {
			this.groundTruthBuffer.shift();
		}
	}

	recordDetection(detection: DetectionResult) {
		this.detectionBuffer.push(detection);
		if (this.detectionBuffer.length > 1000) {
			this.detectionBuffer.shift();
		}
	}

	async evaluateMetrics(windowSeconds: number = 60): Promise<void> {
		const now = Date.now() / 1000;
		const windowStart = now - windowSeconds;

		// Filter events in the window
		const relevantTruths = this.groundTruthBuffer.filter(
			(t) => t.timestamp >= windowStart,
		);
		const relevantDetections = this.detectionBuffer.filter(
			(d) => d.timestamp >= windowStart,
		);

		if (relevantTruths.length === 0) return;

		let tp = 0; // True Positive
		let fp = 0; // False Positive
		let fn = 0; // False Negative
		// tn (True Negative) is less relevant for time-series anomaly detection

		// Simple matching: If an anomaly exists in truth, check if any detection exists in +/- 1s
		for (const truth of relevantTruths) {
			if (truth.isAnomaly) {
				const detected = relevantDetections.some(
					(d) => Math.abs(d.timestamp - truth.timestamp) < 2 && d.isAnomaly,
				);
				if (detected) {
					tp++;
				} else {
					fn++;
				}
			} else {
				// Normal behavior
				const falseAlarm = relevantDetections.some(
					(d) => Math.abs(d.timestamp - truth.timestamp) < 2 && d.isAnomaly,
				);
				if (falseAlarm) {
					fp++;
				}
			}
		}

		// Calculate Metrics
		const precision = tp + fp === 0 ? 0 : tp / (tp + fp);
		const recall = tp + fn === 0 ? 0 : tp / (tp + fn);
		const f1Score =
			precision + recall === 0
				? 0
				: (2 * precision * recall) / (precision + recall);

		// Save to DB
		await saveEvaluationMetrics(
			Math.floor(now),
			Math.round(precision * 100),
			Math.round(recall * 100),
			Math.round(f1Score * 100),
			relevantTruths[0]?.scenarioName || "unknown",
		);

		logger.info("Evaluation metrics updated", {
			f1: Number(f1Score.toFixed(4)),
			precision: Number(precision.toFixed(4)),
			recall: Number(recall.toFixed(4)),
		});
	}

	async getHistory(limit: number = 20) {
		return await getLatestEvaluationMetrics(limit);
	}
}
