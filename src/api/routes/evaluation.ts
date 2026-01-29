import { Hono } from "hono";
import type { EvaluationService } from "../../services/evaluation-service";

const app = new Hono();

declare module "hono" {
	interface ContextVariableMap {
		evaluationService: EvaluationService;
	}
}

app.get("/metrics", async (c) => {
	const evaluationService = c.get("evaluationService");
	const limit = Number(c.req.query("limit") || "20");
	const metrics = await evaluationService.getHistory(limit);
	return c.json({ metrics });
});

export const evaluationRoutes = app;
