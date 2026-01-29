import { Hono } from "hono";
import type { Simulator } from "../../simulation/simulator";

const app = new Hono();

// Type definitions for Hono context
declare module "hono" {
	interface ContextVariableMap {
		simulator: Simulator;
	}
}

app.get("/scenarios", (c) => {
	const simulator = c.get("simulator");
	return c.json({ scenarios: simulator.listScenarios() });
});

app.post("/scenarios/:name/start", (c) => {
	const simulator = c.get("simulator");
	const name = c.req.param("name");

	try {
		simulator.startScenario(name);
		return c.json({ status: "started", scenario: name });
	} catch (error) {
		return c.json({ error: (error as Error).message }, 400);
	}
});

app.post("/scenarios/stop", (c) => {
	const simulator = c.get("simulator");
	simulator.stopScenario();
	return c.json({ status: "stopped" });
});

app.get("/status", (c) => {
	const simulator = c.get("simulator");
	return c.json(simulator.getStatus());
});

export const simulationRoutes = app;
