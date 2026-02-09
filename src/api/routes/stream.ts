import { Hono } from "hono";
import { z } from "zod";
import type { Tier2Service } from "../../services/tier2-service";
import { logger } from "../../utils/logger";

const app = new Hono();

declare module "hono" {
	interface ContextVariableMap {
		tier2Service: Tier2Service;
	}
}

const AnomalyBatchSchema = z.object({
	signals: z.array(
		z.union([
			z.object({
				t: z.number(),
				u: z.string(),
				score: z.number(),
				severity: z.number(),
				type: z.number(),
			}),
			z.object({
				schema_version: z.number().int().optional(),
				entity_hash: z.number(),
				timestamp: z.number(),
				score: z.number(),
				severity: z.number(),
				primary_detector: z.number().int(),
				detectors_fired: z.number().int(),
				confidence: z.number(),
				detector_scores: z.array(z.number()),
			}),
		]),
	),
});

// Endpoint for Gatekeeper to push anomalies
app.post("/tier2/anomalies", async (c) => {
	const tier2 = c.get("tier2Service") as Tier2Service;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = AnomalyBatchSchema.safeParse(body);
	if (!result.success) {
		return c.json(
			{ error: "Invalid anomaly batch", details: result.error },
			400,
		);
	}

	logger.info("Accepted anomaly batch", { count: result.data.signals.length });

	// Async processing
	c.executionCtx.waitUntil(tier2.processAnomalyBatch(result.data.signals));

	return c.json({ status: "accepted" });
});

export const streamRoutes = app;
