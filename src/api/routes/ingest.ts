import { Hono } from "hono";
import { z } from "zod";
import type { IngestionService } from "../../services/ingestion-service";
import type { Tier1Engine } from "../../services/tier1-engine";

const app = new Hono();

// Type definitions for Hono context
declare module "hono" {
	interface ContextVariableMap {
		ingestionService: IngestionService;
		tier1Engine: Tier1Engine;
	}
}

// Validation schemas
const BatchRequestSchema = z.object({
	logs: z.array(z.any()),
});

app.post("/stream", async (c) => {
	const ingestionService = c.get("ingestionService") as IngestionService;
	const body = await c.req.json();

	const result = BatchRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const summary = await ingestionService.ingestLogBatch(result.data.logs);

	return c.json({
		status: "ok",
		tier1_ingested: summary.totalLogs,
	});
});

export const ingestRoutes = app;
