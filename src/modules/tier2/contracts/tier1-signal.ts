import { z } from "zod";

export const Tier1V1SignalSchema = z.object({
	event_id: z.string().min(1),
	schema_version: z.literal(1),
	entity_hash: z.string().min(1),
	timestamp: z.number().int().positive(),
	score: z.number().min(0).max(1),
	severity: z.number().min(0).max(1),
	primary_detector: z.number().int().min(0),
	detectors_fired: z.number().int().min(0),
	confidence: z.number().min(0).max(1),
	detector_scores: z.array(z.number()),
	attributes: z.record(z.string(), z.unknown()).optional(),
});

export const Tier1V1AnomalyBatchSchema = z.object({
	signals: z.array(Tier1V1SignalSchema).min(1),
});

export type Tier1AnomalySignalV1 = z.infer<typeof Tier1V1SignalSchema>;
