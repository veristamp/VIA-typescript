import {
	getSchema,
	listSchemas as listSchemasFromDb,
	saveSchema as saveSchemaToDb,
} from "../db/registry";

export interface SchemaField {
	name: string;
	type: "datetime" | "keyword" | "integer" | "string";
	sourceField: string;
}

export interface LogSchema {
	id?: number;
	sourceName: string;
	fields: SchemaField[];
}

export interface BehavioralProfile {
	frequency: Record<string, number>;
	cardinality: Record<string, number>;
}

export interface DetectSchemaRequest {
	sourceName: string;
	sampleLogs: string[];
}

interface OtlpLog {
	resourceLogs?: Array<{
		resource?: {
			attributes?: Array<{
				key: string;
				value: Record<string, unknown>;
			}>;
		};
		scopeLogs?: Array<{
			logRecords?: Array<{
				timeUnixNano?: string;
				severityText?: string;
				body?: { stringValue?: string };
			}>;
		}>;
	}>;
}

export interface UnifiedSchema {
	structural: LogSchema;
	behavioral: BehavioralProfile | null;
}

export class SchemaService {
	async detectSchema(
		request: DetectSchemaRequest,
	): Promise<UnifiedSchema | null> {
		if (!request.sampleLogs || request.sampleLogs.length === 0) {
			return null;
		}

		let structuralSchema: LogSchema | null = null;

		// 1. Detect Structural Schema
		try {
			const firstLineJson = JSON.parse(request.sampleLogs[0]);
			if (typeof firstLineJson === "object") {
				const resourceData = firstLineJson as OtlpLog;
				const rl = resourceData.resourceLogs?.[0];
				const scope = rl?.scopeLogs?.[0];
				const logRecord = scope?.logRecords?.[0];

				const attrs = rl?.resource?.attributes || [];

				const _svc = (
					_attrs: Array<{ key: string; value: Record<string, unknown> }>,
				) => {
					for (const a of _attrs || []) {
						if (a.key === "service.name") {
							const v = a.value;
							if (v && typeof v === "object") {
								const vals = Object.values(v);
								return vals[0] as string;
							}
						}
					}
					return null;
				};

				const serviceGuess = _svc(attrs);

				const fields: SchemaField[] = [
					{
						name: "timestamp",
						type: "datetime",
						sourceField: logRecord?.timeUnixNano || "",
					},
					{
						name: "level",
						type: "keyword",
						sourceField: logRecord?.severityText || "",
					},
					{
						name: "service",
						type: "keyword",
						sourceField: serviceGuess || "",
					},
					{
						name: "message",
						type: "string",
						sourceField: logRecord?.body?.stringValue || "",
					},
				];

				structuralSchema = {
					sourceName: request.sourceName,
					fields,
				};
			}
		} catch (_error) {
			console.error("Error parsing sample logs (JSON):", _error);
		}

		// Fallback to BGL regex if JSON parsing failed
		if (!structuralSchema) {
			try {
				const bglDetectPattern =
					/^(?<unix_ts>\d+)\s+(?<date>\S+)\s+(?<node>\S+)\s+(?<device>\S+)\s+(?<component>RAS)\s+(?<sub_component>\w+)\s+(?<level>\w+)\s+(?<message>.*)$/;

				const match = bglDetectPattern.exec(request.sampleLogs[0].trim());
				if (match) {
					const fields: SchemaField[] = [
						{
							name: "timestamp",
							type: "datetime",
							sourceField: "unix_ts",
						},
						{
							name: "level",
							type: "keyword",
							sourceField: "level",
						},
						{
							name: "service",
							type: "keyword",
							sourceField: "node",
						},
						{
							name: "message",
							type: "string",
							sourceField: "message",
						},
					];

					structuralSchema = {
						sourceName: request.sourceName,
						fields,
					};
				}
			} catch (_error) {
				console.error("Error parsing BGL format:", _error);
			}
		}

		if (!structuralSchema) {
			return null;
		}

		// 2. Detect Behavioral Profile
		const behavioralProfile = await this.detectBehavioralProfile(
			request.sampleLogs,
		);

		return {
			structural: structuralSchema,
			behavioral: behavioralProfile,
		};
	}

	async saveSchema(schema: UnifiedSchema): Promise<UnifiedSchema> {
		await saveSchemaToDb(
			schema.structural.sourceName,
			{ fields: schema.structural.fields },
			schema.behavioral || undefined,
		);
		return schema;
	}

	async getSchema(sourceName: string): Promise<UnifiedSchema | null> {
		const result = await getSchema(sourceName);

		if (!result) {
			return null;
		}

		const resultFields =
			(result.schemaJson as { fields: SchemaField[] }).fields || [];

		return {
			structural: {
				id: result.id,
				sourceName: result.sourceName,
				fields: resultFields,
			},
			behavioral: (result.behavioralProfile as BehavioralProfile) || null,
		};
	}

	async listSchemas(): Promise<string[]> {
		const result = await listSchemasFromDb();
		return result;
	}

	async detectBehavioralProfile(
		sampleLogs: string[],
	): Promise<BehavioralProfile | null> {
		if (!sampleLogs || sampleLogs.length === 0) {
			return null;
		}

		const frequency: Record<string, number> = {};
		const cardinality: Record<string, number> = {};

		for (const log of sampleLogs) {
			try {
				const parsed = JSON.parse(log);
				const service =
					parsed.resource?.attributes?.["service.name"] || "unknown";
				const level = parsed.severityText || "INFO";
				const body = parsed.body?.stringValue || "";

				const key = `${service}:${level}`;
				frequency[key] = (frequency[key] || 0) + 1;

				const values =
					body.match(/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/g) ||
					[];
				const uniqueValues = new Set(values);
				cardinality[key] = (cardinality[key] || 0) + uniqueValues.size;
			} catch {
				console.error("Error parsing log for behavioral profile:", log);
			}
		}

		return {
			frequency,
			cardinality,
		};
	}
}
