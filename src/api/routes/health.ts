import { Hono } from "hono";

const app = new Hono();

app.get("/", (c) => {
	return c.json({
		status: "ok",
		service: "via-backend",
		version: "2.0.0",
	});
});

app.get("/health", (c) => {
	return c.json({
		status: "healthy",
		timestamp: new Date().toISOString(),
	});
});

export const healthRoutes = app;
