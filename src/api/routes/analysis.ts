import { Hono } from "hono";
import { z } from "zod";
import type { ForensicAnalysisService } from "../../services/forensic-analysis-service";

const app = new Hono();

// Type definitions for Hono context
declare module "hono" {
	interface ContextVariableMap {
		forensicAnalysisService: ForensicAnalysisService;
	}
}

// Validation schemas
const FindClustersRequestSchema = z.object({
	start_ts: z.number().int(),
	end_ts: z.number().int(),
	text_filter: z.string().optional(),
});

const TriageRequestSchema = z.object({
	positive_ids: z.array(z.string()),
	negative_ids: z.array(z.string()),
	start_ts: z.number().int(),
	end_ts: z.number().int(),
});

app.post("/clusters", async (c) => {
	const forensicAnalysisService = c.get(
		"forensicAnalysisService",
	) as ForensicAnalysisService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = FindClustersRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const clusters = await forensicAnalysisService.findTier2Clusters(
		result.data.start_ts,
		result.data.end_ts,
		result.data.text_filter,
	);

	return c.json({
		clusters,
	});
});

app.post("/triage", async (c) => {
	const forensicAnalysisService = c.get(
		"forensicAnalysisService",
	) as ForensicAnalysisService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = TriageRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const triageResults = await forensicAnalysisService.triageSimilarEvents(
		result.data.positive_ids,
		result.data.negative_ids,
		result.data.start_ts,
		result.data.end_ts,
	);

	return c.json({
		triage_results: triageResults,
	});
});

app.post("/correlate", async (c) => {
	const forensicAnalysisService = c.get(
		"forensicAnalysisService",
	) as ForensicAnalysisService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = FindClustersRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	await forensicAnalysisService.correlateIncidents(
		result.data.start_ts,
		result.data.end_ts,
	);

	return c.json({
		status: "ok",
		message: "Incident correlation completed",
	});
});

app.get("/incident/:metaIncidentId", async (c) => {
	const forensicAnalysisService = c.get(
		"forensicAnalysisService",
	) as ForensicAnalysisService;
	const metaIncidentId = c.req.param("metaIncidentId");

	if (!metaIncidentId) {
		return c.json({ error: "metaIncidentId parameter is required" }, 400);
	}

	const graph = await forensicAnalysisService.getIncidentGraph(metaIncidentId);

	return c.json({
		graph,
	});
});

export const analysisRoutes = app;
