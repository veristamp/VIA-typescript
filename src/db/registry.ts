import { and, desc, eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";
import { Pool } from "pg";
import { settings } from "../config/settings";
import type { EvaluationMetric, Patch } from "./schema";
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
