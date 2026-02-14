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

export const tier2Incidents = pgTable("tier2_incidents", {
	id: serial("id").primaryKey(),
	incidentId: text("incident_id").notNull().unique(),
	status: text("status").notNull(),
	entityKey: text("entity_key").notNull(),
	firstSeenTs: integer("first_seen_ts").notNull(),
	lastSeenTs: integer("last_seen_ts").notNull(),
	severityMax: integer("severity_max").notNull(),
	scoreMax: integer("score_max").notNull(),
	confidence: integer("confidence").notNull(),
	evidence: jsonb("evidence").notNull(),
	policyVersion: text("policy_version").notNull(),
	updatedAt: timestamp("updated_at").defaultNow(),
	createdAt: timestamp("created_at").defaultNow(),
});

export const tier2Decisions = pgTable("tier2_decisions", {
	id: serial("id").primaryKey(),
	incidentId: text("incident_id").notNull(),
	decision: text("decision").notNull(),
	reason: text("reason").notNull(),
	confidence: integer("confidence").notNull(),
	policyVersion: text("policy_version").notNull(),
	createdAt: timestamp("created_at").defaultNow(),
});

export const tier2DeadLetters = pgTable("tier2_dead_letters", {
	id: serial("id").primaryKey(),
	eventId: text("event_id").notNull(),
	reason: text("reason").notNull(),
	payload: jsonb("payload").notNull(),
	createdAt: timestamp("created_at").defaultNow(),
});

export const tier1PolicyArtifacts = pgTable("tier1_policy_artifacts", {
	id: serial("id").primaryKey(),
	policyVersion: text("policy_version").notNull().unique(),
	status: text("status").notNull(), // draft | active | rolled_back
	compiledJson: jsonb("compiled_json").notNull(),
	featureFlags: jsonb("feature_flags").notNull(),
	rollbackOf: text("rollback_of"),
	createdAt: timestamp("created_at").defaultNow(),
});

export const tier1PolicyMetrics = pgTable("tier1_policy_metrics", {
	id: serial("id").primaryKey(),
	policyVersion: text("policy_version").notNull(),
	windowStartTs: integer("window_start_ts").notNull(),
	windowEndTs: integer("window_end_ts").notNull(),
	precision: integer("precision").notNull(),
	recall: integer("recall").notNull(),
	latencyP95: integer("latency_p95").notNull(),
	latencyP99: integer("latency_p99").notNull(),
	dropRate: integer("drop_rate").notNull(),
	createdAt: timestamp("created_at").defaultNow(),
});

// Evaluation metrics for benchmark framework
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
export type Tier2Incident = typeof tier2Incidents.$inferSelect;
export type Tier2Decision = typeof tier2Decisions.$inferSelect;
export type Tier2DeadLetter = typeof tier2DeadLetters.$inferSelect;
export type Tier1PolicyArtifact = typeof tier1PolicyArtifacts.$inferSelect;
export type Tier1PolicyMetric = typeof tier1PolicyMetrics.$inferSelect;
