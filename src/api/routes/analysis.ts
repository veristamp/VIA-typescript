import { Hono } from "hono";
import { z } from "zod";
import type { ForensicAnalysisService } from "../../services/forensic-analysis-service";
import type { IncidentService } from "../../services/incident-service";
import type { Tier2QueueService } from "../../services/tier2-queue-service";

const app = new Hono();

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

	return c.json({ clusters });
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

	return c.json({ triage_results: triageResults });
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

	return c.json({ graph });
});

app.get("/incidents", async (c) => {
	const incidentService = c.get("incidentService") as IncidentService;
	const parsed = Number(c.req.query("limit") ?? "50");
	const limit = Number.isFinite(parsed)
		? Math.min(Math.max(parsed, 1), 500)
		: 50;
	const incidents = await incidentService.listIncidents(limit);
	return c.json({ incidents });
});

app.get("/incidents/run/:runId", async (c) => {
	const incidentService = c.get("incidentService") as IncidentService;
	const runId = c.req.param("runId");
	if (!runId) {
		return c.json({ error: "runId parameter is required" }, 400);
	}
	const parsed = Number(c.req.query("limit") ?? "20000");
	const limit = Number.isFinite(parsed)
		? Math.min(Math.max(parsed, 1), 100000)
		: 20000;
	const incidents = await incidentService.listIncidentsForRun(runId, limit);
	return c.json({ incidents });
});

app.get("/incidents/:incidentId", async (c) => {
	const incidentService = c.get("incidentService") as IncidentService;
	const incidentId = c.req.param("incidentId");
	if (!incidentId) {
		return c.json({ error: "incidentId parameter is required" }, 400);
	}
	const result = await incidentService.getIncident(incidentId);
	if (!result) {
		return c.json({ error: "incident not found" }, 404);
	}
	return c.json(result);
});

app.post("/incidents/:incidentId/enrich", async (c) => {
	const forensicAnalysisService = c.get(
		"forensicAnalysisService",
	) as ForensicAnalysisService;
	const incidentId = c.req.param("incidentId");
	const body = await c.req.json().catch(() => ({}));
	const { trace_id, start_ts, end_ts } = body;

	if (!incidentId || !trace_id || !start_ts || !end_ts) {
		return c.json(
			{ error: "incidentId, trace_id, start_ts, and end_ts are required" },
			400,
		);
	}

	const enrichment = await forensicAnalysisService.enrichIncident(
		incidentId,
		trace_id,
		start_ts,
		end_ts,
	);

	return c.json(enrichment);
});

app.get("/pipeline/stats", (c) => {
	const queue = c.get("tier2QueueService") as Tier2QueueService;
	return c.json({ queue: queue.getStats() });
});

app.get("/pipeline/dead-letters", async (c) => {
	const incidentService = c.get("incidentService") as IncidentService;
	const parsed = Number(c.req.query("limit") ?? "50");
	const limit = Number.isFinite(parsed)
		? Math.min(Math.max(parsed, 1), 500)
		: 50;
	const deadLetters = await incidentService.getDeadLetters(limit);
	return c.json({ dead_letters: deadLetters });
});

export const analysisRoutes = app;
