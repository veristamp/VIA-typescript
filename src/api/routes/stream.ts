import { Hono } from "hono";
import { Tier1V1AnomalyBatchSchema } from "../../modules/tier2/contracts/tier1-signal";
import type { Tier2QueueService } from "../../services/tier2-queue-service";
import { logger } from "../../utils/logger";

const app = new Hono();

declare module "hono" {
	interface ContextVariableMap {
		tier2QueueService: Tier2QueueService;
	}
}

app.post("/tier2/anomalies", async (c) => {
	const queue = c.get("tier2QueueService") as Tier2QueueService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = Tier1V1AnomalyBatchSchema.safeParse(body);
	if (!result.success) {
		return c.json(
			{ error: "Invalid anomaly batch", details: result.error },
			400,
		);
	}

	const enqueueResult = queue.enqueue(result.data.signals);
	if (!enqueueResult.accepted) {
		logger.warn("Rejected anomaly batch", {
			eventId: enqueueResult.eventId,
			reason: enqueueResult.reason,
		});
		return c.json(
			{
				status: "rejected",
				event_id: enqueueResult.eventId,
				reason: enqueueResult.reason,
			},
			429,
		);
	}

	logger.info("Accepted anomaly batch", {
		eventId: enqueueResult.eventId,
		count: result.data.signals.length,
	});

	return c.json({
		status: "accepted",
		event_id: enqueueResult.eventId,
	});
});

export const streamRoutes = app;
