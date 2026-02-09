import { and, desc, eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";
import { Pool } from "pg";
import { settings } from "../config/settings";
import type {
	EvaluationMetric,
	Patch,
	Tier2DeadLetter,
	Tier2Decision,
	Tier2Incident,
} from "./schema";
import * as schema from "./schema";

const pool = new Pool({
	host: settings.postgres.host,
	port: settings.postgres.port,
	database: settings.postgres.database,
	user: settings.postgres.user,
	password: settings.postgres.password,
	ssl: false,
});

export const db = drizzle(pool, { schema });

export async function initializeRegistry(): Promise<void> {
	await pool.query(`
		CREATE TABLE IF NOT EXISTS schemas (
			id SERIAL PRIMARY KEY,
			source_name TEXT NOT NULL UNIQUE,
			schema_json JSONB NOT NULL,
			behavioral_profile JSONB,
			created_at TIMESTAMP DEFAULT NOW(),
			updated_at TIMESTAMP DEFAULT NOW()
		);

		CREATE TABLE IF NOT EXISTS patch_registry (
			id SERIAL PRIMARY KEY,
			rhythm_hash TEXT NOT NULL UNIQUE,
			rule TEXT NOT NULL,
			reason TEXT,
			created_ts INTEGER,
			is_active BOOLEAN DEFAULT TRUE
		);

		CREATE TABLE IF NOT EXISTS incident_graph (
			id SERIAL PRIMARY KEY,
			meta_incident_id TEXT NOT NULL,
			qdrant_point_id TEXT NOT NULL,
			link_type TEXT,
			confidence INTEGER,
			created_at TIMESTAMP DEFAULT NOW()
		);

		CREATE TABLE IF NOT EXISTS evaluation_metrics (
			id SERIAL PRIMARY KEY,
			timestamp INTEGER NOT NULL,
			precision INTEGER,
			recall INTEGER,
			f1_score INTEGER,
			scenario_name TEXT
		);

		CREATE TABLE IF NOT EXISTS tier2_incidents (
			id SERIAL PRIMARY KEY,
			incident_id TEXT NOT NULL UNIQUE,
			status TEXT NOT NULL,
			entity_key TEXT NOT NULL,
			first_seen_ts INTEGER NOT NULL,
			last_seen_ts INTEGER NOT NULL,
			severity_max INTEGER NOT NULL,
			score_max INTEGER NOT NULL,
			confidence INTEGER NOT NULL,
			evidence JSONB NOT NULL,
			policy_version TEXT NOT NULL,
			updated_at TIMESTAMP DEFAULT NOW(),
			created_at TIMESTAMP DEFAULT NOW()
		);

		CREATE TABLE IF NOT EXISTS tier2_decisions (
			id SERIAL PRIMARY KEY,
			incident_id TEXT NOT NULL,
			decision TEXT NOT NULL,
			reason TEXT NOT NULL,
			confidence INTEGER NOT NULL,
			policy_version TEXT NOT NULL,
			created_at TIMESTAMP DEFAULT NOW()
		);

		CREATE TABLE IF NOT EXISTS tier2_dead_letters (
			id SERIAL PRIMARY KEY,
			event_id TEXT NOT NULL,
			reason TEXT NOT NULL,
			payload JSONB NOT NULL,
			created_at TIMESTAMP DEFAULT NOW()
		);
	`);
}

export async function getSchema(sourceName: string) {
	return db.query.schemas.findFirst({
		where: eq(schema.schemas.sourceName, sourceName),
	});
}

export async function saveSchema(
	sourceName: string,
	schemaJson: object,
	behavioralProfile?: object,
): Promise<void> {
	await db
		.insert(schema.schemas)
		.values({
			sourceName,
			schemaJson,
			behavioralProfile: behavioralProfile || null,
		})
		.onConflictDoUpdate({
			target: schema.schemas.sourceName,
			set: { schemaJson, behavioralProfile },
		});
}

export async function listSchemas(): Promise<string[]> {
	const result = await db.query.schemas.findMany({
		columns: {
			sourceName: true,
		},
	});

	return result.map((row) => row.sourceName);
}

// Patch registry functions
export async function getActivePatches(): Promise<Patch[]> {
	return db.query.patchRegistry.findMany({
		where: and(
			eq(schema.patchRegistry.rule, "ALLOW_LIST"),
			eq(schema.patchRegistry.isActive, true),
		),
	});
}

export async function patchAnomaly(
	rhythmHash: string,
	reason: string,
): Promise<void> {
	await db.insert(schema.patchRegistry).values({
		rhythmHash,
		rule: "ALLOW_LIST",
		reason,
		createdTs: Math.floor(Date.now() / 1000),
		isActive: true,
	});
}

export async function deletePatch(rhythmHash: string): Promise<void> {
	await db
		.update(schema.patchRegistry)
		.set({ isActive: false })
		.where(eq(schema.patchRegistry.rhythmHash, rhythmHash));
}

export async function getAllRules(): Promise<Patch[]> {
	const patches = await db.query.patchRegistry.findMany({
		where: eq(schema.patchRegistry.isActive, true),
	});

	return patches;
}

// Incident graph functions
export async function saveIncidentGraph(
	metaIncidentId: string,
	qdrantPointId: string,
	linkType: string,
	confidence?: number,
): Promise<void> {
	await db.insert(schema.incidentGraph).values({
		metaIncidentId,
		qdrantPointId,
		linkType,
		confidence,
	});
}

export async function getIncidentGraph(metaIncidentId: string) {
	const graph = await db.query.incidentGraph.findMany({
		where: eq(schema.incidentGraph.metaIncidentId, metaIncidentId),
	});

	return graph;
}

// Evaluation metrics functions
export async function saveEvaluationMetrics(
	timestamp: number,
	precision: number,
	recall: number,
	f1Score: number,
	scenarioName?: string,
): Promise<void> {
	await db.insert(schema.evaluationMetrics).values({
		timestamp,
		precision,
		recall,
		f1Score,
		scenarioName,
	});
}

export async function getLatestEvaluationMetrics(
	limit: number = 10,
): Promise<EvaluationMetric[]> {
	const result = await db.query.evaluationMetrics.findMany({
		orderBy: [desc(schema.evaluationMetrics.timestamp)],
		limit,
	});
	return result;
}

export interface UpsertTier2IncidentInput {
	incidentId: string;
	status: string;
	entityKey: string;
	firstSeenTs: number;
	lastSeenTs: number;
	severityMaxPct: number;
	scoreMaxPct: number;
	confidencePct: number;
	evidence: Record<string, unknown>;
	policyVersion: string;
}

export async function upsertTier2Incident(
	input: UpsertTier2IncidentInput,
): Promise<void> {
	await db
		.insert(schema.tier2Incidents)
		.values({
			incidentId: input.incidentId,
			status: input.status,
			entityKey: input.entityKey,
			firstSeenTs: input.firstSeenTs,
			lastSeenTs: input.lastSeenTs,
			severityMax: input.severityMaxPct,
			scoreMax: input.scoreMaxPct,
			confidence: input.confidencePct,
			evidence: input.evidence,
			policyVersion: input.policyVersion,
		})
		.onConflictDoUpdate({
			target: schema.tier2Incidents.incidentId,
			set: {
				status: input.status,
				entityKey: input.entityKey,
				firstSeenTs: input.firstSeenTs,
				lastSeenTs: input.lastSeenTs,
				severityMax: input.severityMaxPct,
				scoreMax: input.scoreMaxPct,
				confidence: input.confidencePct,
				evidence: input.evidence,
				policyVersion: input.policyVersion,
				updatedAt: new Date(),
			},
		});
}

export async function saveTier2Decision(
	incidentId: string,
	decision: string,
	reason: string,
	confidencePct: number,
	policyVersion: string,
): Promise<void> {
	await db.insert(schema.tier2Decisions).values({
		incidentId,
		decision,
		reason,
		confidence: confidencePct,
		policyVersion,
	});
}

export async function saveDeadLetter(
	eventId: string,
	reason: string,
	payload: Record<string, unknown>,
): Promise<void> {
	await db.insert(schema.tier2DeadLetters).values({
		eventId,
		reason,
		payload,
	});
}

export async function listTier2Incidents(
	limit: number,
): Promise<Tier2Incident[]> {
	return db.query.tier2Incidents.findMany({
		orderBy: [desc(schema.tier2Incidents.lastSeenTs)],
		limit,
	});
}

export async function getTier2IncidentById(
	incidentId: string,
): Promise<Tier2Incident | undefined> {
	return db.query.tier2Incidents.findFirst({
		where: eq(schema.tier2Incidents.incidentId, incidentId),
	});
}

export async function listTier2Decisions(
	incidentId: string,
	limit: number,
): Promise<Tier2Decision[]> {
	return db.query.tier2Decisions.findMany({
		where: eq(schema.tier2Decisions.incidentId, incidentId),
		orderBy: [desc(schema.tier2Decisions.id)],
		limit,
	});
}

export async function getLatestDeadLetters(
	limit: number,
): Promise<Tier2DeadLetter[]> {
	return db.query.tier2DeadLetters.findMany({
		orderBy: [desc(schema.tier2DeadLetters.id)],
		limit,
	});
}
