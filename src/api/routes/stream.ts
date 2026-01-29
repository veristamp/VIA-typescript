import { Hono } from "hono";
import { z } from "zod";
import type { AnomalyProfile } from "../../core/anomaly-profile";
import type { PromotionService } from "../../services/promotion-service";
import type { Tier1Engine } from "../../services/tier1-engine";

const app = new Hono();

declare module "hono" {
	interface ContextVariableMap {
		promotionService: PromotionService;
		tier1Engine: Tier1Engine;
	}
}

const PromoteRequestSchema = z.object({
	anomalies: z.array(
		z.object({
			rhythm_hash: z.string(),
			service: z.string(),
			severity: z.string(),
			template: z.string(),
			count: z.number().int(),
			first_seen: z.number().int(),
			last_seen: z.number().int(),
			anomaly_score: z.number(),
		}),
	),
});

app.post("/promote", async (c) => {
	const promotionService = c.get("promotionService") as PromotionService;
	const body = await c.req.json();

	const result = PromoteRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const anomalySignals = result.data.anomalies.map((a) => ({
		rhythmHash: a.rhythm_hash,
		service: a.service,
		severity: a.severity,
		anomalyType: "frequency" as const,
		confidence: a.anomaly_score,
		context: `Template: ${a.template}, Count: ${a.count}`,
		timestamp: a.last_seen,
		metadata: {
			template: a.template,
			count: a.count,
			firstSeen: a.first_seen,
			lastSeen: a.last_seen,
		},
	}));

	await promotionService.promoteAnomalies(anomalySignals);

	return c.json({
		status: "ok",
		message: "Anomalies promoted to Tier-2",
	});
});

app.get("/anomalies", async (c) => {
	const tier1Engine = c.get("tier1Engine") as Tier1Engine;

	const profiles = tier1Engine.getAllProfiles();
	const anomalies = profiles.filter((p: AnomalyProfile) => {
		const stats = p.getStats();
		return stats.eventCount > 0;
	});

	return c.json({
		anomalies,
	});
});

app.get("/profiles", async (c) => {
	const tier1Engine = c.get("tier1Engine") as Tier1Engine;

	const profiles = tier1Engine.getAllProfiles();

	return c.json({
		profiles,
	});
});

export const streamRoutes = app;
