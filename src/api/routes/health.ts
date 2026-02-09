import { Hono } from "hono";

const app = new Hono();
const SERVICE_VERSION = "2.2.0";
const SIGNAL_SCHEMA_VERSION = 2;

app.get("/", (c) => {
	return c.json({
		status: "ok",
		service: "via-backend",
		version: SERVICE_VERSION,
		signal_schema_version: SIGNAL_SCHEMA_VERSION,
	});
});

app.get("/health", (c) => {
	return c.json({
		status: "healthy",
		version: SERVICE_VERSION,
		signal_schema_version: SIGNAL_SCHEMA_VERSION,
		timestamp: new Date().toISOString(),
	});
});

export const healthRoutes = app;
