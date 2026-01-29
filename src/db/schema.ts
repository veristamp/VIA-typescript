import {
	boolean,
	integer,
	jsonb,
	pgTable,
	serial,
	text,
	timestamp,
} from "drizzle-orm/pg-core";

// Schema registry for dynamic log parsing schemas
export const schemas = pgTable("schemas", {
	id: serial("id").primaryKey(),
	sourceName: text("source_name").notNull().unique(),
	schemaJson: jsonb("schema_json").notNull(),
	behavioralProfile: jsonb("behavioral_profile"),
	createdAt: timestamp("created_at").defaultNow(),
	updatedAt: timestamp("updated_at").defaultNow(),
});

// Patch registry for control loop (suppression and patching)
export const patchRegistry = pgTable("patch_registry", {
	id: serial("id").primaryKey(),
	rhythmHash: text("rhythm_hash").notNull().unique(),
	rule: text("rule").notNull(), // 'ALLOW_LIST', 'BLOCK', etc.
	reason: text("reason"),
	createdTs: integer("created_ts"),
	isActive: boolean("is_active").default(true),
});

// Incident graph for Tier-2 correlation (new in v2)
export const incidentGraph = pgTable("incident_graph", {
	id: serial("id").primaryKey(),
	metaIncidentId: text("meta_incident_id").notNull(),
	qdrantPointId: text("qdrant_point_id").notNull(),
	linkType: text("link_type"), // 'temporal', 'trace', 'semantic'
	confidence: integer("confidence"),
	createdAt: timestamp("created_at").defaultNow(),
});

// Evaluation metrics for simulation framework (new in v2)
export const evaluationMetrics = pgTable("evaluation_metrics", {
	id: serial("id").primaryKey(),
	timestamp: integer("timestamp").notNull(),
	precision: integer("precision"),
	recall: integer("recall"),
	f1Score: integer("f1_score"),
	scenarioName: text("scenario_name"),
});

// Type exports
export type Schema = typeof schemas.$inferSelect;
export type Patch = typeof patchRegistry.$inferSelect;
export type IncidentGraph = typeof incidentGraph.$inferSelect;
export type EvaluationMetric = typeof evaluationMetrics.$inferSelect;
