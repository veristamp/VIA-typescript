import { Hono } from "hono";
import { z } from "zod";
import type { Tier2Service } from "../../services/tier2-service";

const app = new Hono();

declare module "hono" {
	interface ContextVariableMap {
		tier2Service: Tier2Service;
	}
}

const AnomalyBatchSchema = z.object({
	signals: z.array(z.object({
        t: z.number(),
        u: z.string(),
        score: z.number(),
        severity: z.number(),
        type: z.number()
    }))
});

// Endpoint for Gatekeeper to push anomalies
app.post("/tier2/anomalies", async (c) => {
	const tier2 = c.get("tier2Service") as Tier2Service;
	const body = await c.req.json();

	const result = AnomalyBatchSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid anomaly batch", details: result.error }, 400);
	}

    // Async processing
	c.executionCtx.waitUntil(tier2.processAnomalyBatch(result.data.signals));

	return c.json({ status: "accepted" });
});

export const streamRoutes = app;
