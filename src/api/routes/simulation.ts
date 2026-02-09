import { Hono } from "hono";
import { z } from "zod";
import { RustSimulationEngine } from "../../rust-bridge";

const app = new Hono();
const simulation = new RustSimulationEngine();

// Start with baseline traffic.
simulation.addNormalTraffic(50.0);

const ScenarioSchema = z.object({
	type: z.enum([
		"normal",
		"memory_leak",
		"cpu_spike",
		"credential_stuffing",
		"sql_injection",
		"port_scan",
	]),
	intensity: z.number().positive(),
});

app.get("/tick", (c) => {
	const logs = simulation.tick(100_000_000);
	return c.body(logs, 200, { "content-type": "application/json" });
});

app.post("/scenario", async (c) => {
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = ScenarioSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const { type, intensity } = result.data;

	switch (type) {
		case "normal":
			simulation.addNormalTraffic(intensity);
			break;
		case "memory_leak":
			simulation.addMemoryLeak(intensity);
			break;
		case "cpu_spike":
			simulation.addCpuSpike(intensity);
			break;
		case "credential_stuffing":
			simulation.addCredentialStuffing(intensity);
			break;
		case "sql_injection":
			simulation.addSqlInjection(intensity);
			break;
		case "port_scan":
			simulation.addPortScan(intensity);
			break;
	}

	return c.json({
		message: `Scenario ${type} added with intensity ${intensity}`,
	});
});

app.post("/reset", (c) => {
	simulation.reset();
	simulation.addNormalTraffic(50.0);
	return c.json({ message: "Simulation reset to baseline" });
});

export const simulationRoutes = app;
