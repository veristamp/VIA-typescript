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

const CompilePolicyRequestSchema = z.object({
	limit: z.number().int().positive().max(500).optional(),
});

const PublishPolicyRequestSchema = z.object({
	policy_version: z.string().min(1),
});

const RollbackPolicyRequestSchema = z.object({
	target_version: z.string().min(1),
	reason: z.string().min(1),
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

app.post("/policy/compile", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const body = await c.req.json().catch(() => ({}));
	const result = CompilePolicyRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const artifact = await controlService.compilePolicy(result.data.limit ?? 250);
	return c.json({
		status: "ok",
		policy_version: artifact.policyVersion,
		rule_count: artifact.snapshot.rules.length,
	});
});

app.post("/policy/publish", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}
	const result = PublishPolicyRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}
	await controlService.publishPolicy(result.data.policy_version);
	return c.json({ status: "ok", policy_version: result.data.policy_version });
});

app.post("/policy/rollback", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}
	const result = RollbackPolicyRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}
	const rollbackVersion = await controlService.rollbackPolicy(
		result.data.target_version,
		result.data.reason,
	);
	return c.json({ status: "ok", policy_version: rollbackVersion });
});

app.get("/policy/current", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const policy = await controlService.getCurrentPolicy();
	if (!policy) {
		return c.json({ policy: null }, 404);
	}
	return c.json({ policy });
});

app.get("/policy/:version", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const version = c.req.param("version");
	if (!version) {
		return c.json({ error: "version is required" }, 400);
	}
	const policy = await controlService.getPolicyByVersion(version);
	if (!policy) {
		return c.json({ policy: null }, 404);
	}
	return c.json({ policy });
});

app.get("/policy", async (c) => {
	const controlService = c.get("controlService") as ControlService;
	const parsed = Number(c.req.query("limit") ?? "50");
	const limit = Number.isFinite(parsed)
		? Math.min(Math.max(parsed, 1), 500)
		: 50;
	const policies = await controlService.listPolicies(limit);
	return c.json({ policies });
});

export const controlRoutes = app;
