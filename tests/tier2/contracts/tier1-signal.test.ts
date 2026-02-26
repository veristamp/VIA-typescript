import { describe, expect, it } from "bun:test";
import {
	Tier1V1AnomalyBatchSchema,
	normalizeTier1Severity,
} from "../../../src/modules/tier2/contracts/tier1-signal";

describe("Tier1 signal contract", () => {
	it("accepts valid v1 anomaly batch payload", () => {
		const parsed = Tier1V1AnomalyBatchSchema.safeParse({
			signals: [
				{
					event_id: "evt-1",
					schema_version: 1,
					entity_hash: "123",
					timestamp: 1738000000000000000,
					score: 0.9,
					severity: 4,
					primary_detector: 2,
					detectors_fired: 4,
					confidence: 0.88,
					detector_scores: [0.9, 0.7],
				},
			],
		});
		expect(parsed.success).toBeTrue();
	});

	it("rejects empty signal batch", () => {
		const parsed = Tier1V1AnomalyBatchSchema.safeParse({ signals: [] });
		expect(parsed.success).toBeFalse();
	});

	it("normalizes Tier-1 severity enum into Tier-2 scale", () => {
		expect(normalizeTier1Severity(0, 1)).toBe(0);
		expect(normalizeTier1Severity(1, 1)).toBe(0.25);
		expect(normalizeTier1Severity(2, 1)).toBe(0.5);
		expect(normalizeTier1Severity(3, 1)).toBe(0.75);
		expect(normalizeTier1Severity(4, 1)).toBe(1);
	});
});
