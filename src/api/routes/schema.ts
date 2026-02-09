import { Hono } from "hono";
import { z } from "zod";
import type {
	DetectSchemaRequest,
	SchemaService,
	UnifiedSchema,
} from "../../services/schema-service";

const app = new Hono();

// Type definitions for Hono context
declare module "hono" {
	interface ContextVariableMap {
		schemaService: SchemaService;
	}
}

// Validation schemas
const DetectSchemaRequestSchema = z.object({
	sourceName: z.string(),
	sampleLogs: z.array(z.string()),
});

// For saving, we expect the unified structure
const SaveSchemaRequestSchema = z.object({
	structural: z.object({
		sourceName: z.string(),
		fields: z.array(z.any()),
		id: z.number().optional(),
	}),
	behavioral: z
		.object({
			frequency: z.record(z.string(), z.number()),
			cardinality: z.record(z.string(), z.number()),
		})
		.nullable(),
});

app.post("/detect", async (c) => {
	const schemaService = c.get("schemaService") as SchemaService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = DetectSchemaRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	const schema = await schemaService.detectSchema(
		result.data as DetectSchemaRequest,
	);

	if (!schema) {
		return c.json({ error: "Failed to detect schema" }, 500);
	}

	return c.json({
		schema,
	});
});

app.post("/save", async (c) => {
	const schemaService = c.get("schemaService") as SchemaService;
	const body = await c.req.json().catch(() => null);
	if (!body) {
		return c.json({ error: "Invalid JSON body" }, 400);
	}

	const result = SaveSchemaRequestSchema.safeParse(body);
	if (!result.success) {
		return c.json({ error: "Invalid request", details: result.error }, 400);
	}

	// We cast to UnifiedSchema because Zod validation ensures structure
	const schema = await schemaService.saveSchema(
		result.data as unknown as UnifiedSchema,
	);

	return c.json({
		status: "ok",
		schema,
	});
});

app.get("/:sourceName", async (c) => {
	const schemaService = c.get("schemaService") as SchemaService;
	const sourceName = c.req.param("sourceName");

	if (!sourceName) {
		return c.json({ error: "sourceName parameter is required" }, 400);
	}

	const schema = await schemaService.getSchema(sourceName);

	if (!schema) {
		return c.json({ error: "Schema not found" }, 404);
	}

	return c.json({
		schema,
	});
});

app.get("/", async (c) => {
	const schemaService = c.get("schemaService") as SchemaService;
	const schemas = await schemaService.listSchemas();

	return c.json({
		schemas,
	});
});

export const schemaRoutes = app;
