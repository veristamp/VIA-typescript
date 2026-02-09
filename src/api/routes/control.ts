import { Hono } from "hono";
import { z } from "zod";
import type { ControlService } from "../../services/control-service";

const app = new Hono();

// Type definitions for Hono context
declare module "hono" {
	interface ContextVariableMap {
		controlService: ControlService;
	}
}

// Validation schemas
const SuppressRequestSchema = z.object({
	rhythm_hash: z.string(),
	duration_sec: z.number().int().positive(),
});

const PatchRequestSchema = z.object({
	rhythm_hash: z.string(),
	reason: z.string(),
	context_logs: z.array(z.string()),
});

app.post("/suppress", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = SuppressRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	await controlService.suppressAnomaly(
		result.data.rhythm_hash,
		result.data.duration_sec,
	);

	return c.json({
		status: "ok",
		message: `Anomaly suppressed for ${result.data.duration_sec} seconds`,
	});
});

app.post("/patch", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = PatchRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	await controlService.patchAnomaly(
		result.data.rhythm_hash,
		result.data.reason,
		result.data.context_logs,
	);

	return c.json({
		status: "ok",
		message: "Anomaly patched permanently",
	});
});

app.delete("/patch/:rhythmHash", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const rhythmHash = c.req.param("rhythmHash");

	if (!rhythmHash) {
		return c.json({ error: "rhythmHash parameter is required" }, 400);
	}

	await controlService.deletePatch(rhythmHash);

	return c.json({
		status: "ok",
		message: "Patch deleted",
	});
});

app.delete("/suppress/:rhythmHash", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const rhythmHash = c.req.param("rhythmHash");

	if (!rhythmHash) {
		return c.json({ error: "rhythmHash parameter is required" }, 400);
	}

	await controlService.deleteSuppression(rhythmHash);

	return c.json({
		status: "ok",
		message: "Suppression deleted",
	});
});

app.get("/rules", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const rules = await controlService.getAllRules();

	return c.json({
		rules,
	});
});

export const controlRoutes = app;
