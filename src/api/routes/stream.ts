import { Hono } from "hono";
import { streamSSE } from "hono/streaming";
import { Tier1V1AnomalyBatchSchema } from "../../utils/normalization";
import type { Tier2QueueService } from "../../services/tier2-queue-service";
import { logger } from "../../utils/logger";

const app = new Hono();

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

app.get("/signals", async (c) => {
	const queue = c.get("tier2QueueService") as Tier2QueueService;

	return streamSSE(c, async (stream) => {
		const onSignals = async (signals: any[]) => {
			await stream.writeSSE({
				data: JSON.stringify(signals),
				event: "signals",
			});
		};

		queue.on("signals", onSignals);

		// Keep-alive heartbeat
		const heartbeat = setInterval(async () => {
			await stream.writeSSE({ data: "ping", event: "ping" });
		}, 15000);

		stream.onAbort(() => {
			queue.off("signals", onSignals);
			clearInterval(heartbeat);
		});

		// Initial connection message
		await stream.writeSSE({ data: "connected", event: "info" });
	});
});

export const streamRoutes = app;
