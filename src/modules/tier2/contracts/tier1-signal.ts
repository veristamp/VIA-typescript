import { z } from "zod";

export const Tier1V1SignalSchema = z.object({
	event_id: z.string().min(1).optional(),
	schema_version: z.number().int(),
	entity_hash: z.string().min(1),
	timestamp: z.union([z.number(), z.string().min(1)]),
	score: z.number(),
	severity: z.number(),
	primary_detector: z.number().int(),
	detectors_fired: z.number().int(),
	confidence: z.number(),
	detector_scores: z.array(z.number()),
	attributes: z.record(z.string(), z.unknown()).optional(),
});

export const Tier1V1AnomalyBatchSchema = z.object({
	signals: z.array(Tier1V1SignalSchema).min(1),
});

export type Tier1AnomalySignalV1 = z.infer<typeof Tier1V1SignalSchema>;

export function normalizeTier1Severity(
	rawSeverity: number,
	schemaVersion: number,
): number {
	if (!Number.isFinite(rawSeverity) || rawSeverity <= 0) {
		return 0;
	}
	if (
		schemaVersion === 1 &&
		Number.isInteger(rawSeverity) &&
		rawSeverity >= 0 &&
		rawSeverity <= 4
	) {
		return rawSeverity / 4;
	}
	if (rawSeverity <= 1) {
		return rawSeverity;
	}
	return Math.min(1, rawSeverity / 4);
}
