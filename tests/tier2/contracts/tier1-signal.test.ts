import { describe, expect, it } from "bun:test";
import { Tier1V1AnomalyBatchSchema } from "../../../src/modules/tier2/contracts/tier1-signal";

describe("Tier1 signal contract", () => {
	it("accepts valid v1 anomaly batch payload", () => {
		const parsed = Tier1V1AnomalyBatchSchema.safeParse({
			signals: [
				{
					event_id: "evt-1",
					schema_version: 1,
					entity_hash: "123",
					timestamp: 1738000000,
					score: 0.9,
					severity: 1,
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

	it("rejects non-canonical pre-Rust payload values", () => {
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
		expect(parsed.success).toBeFalse();
	});
});
