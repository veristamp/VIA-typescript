import type { LogRecord } from "../types";

export function normalizeLog(raw: unknown): LogRecord {
	const log = raw as {
		resource?: { attributes?: Record<string, unknown> };
		severityText?: string;
		body?: { stringValue?: string };
		attributes?: Record<string, unknown>;
	};

	return {
		id: generateId(),
		timestamp: Date.now() / 1000,
		service: String(log.resource?.attributes?.["service.name"] || "unknown"),
		severity: String(log.severityText || "INFO"),
		body: String(log.body?.stringValue || ""),
		rhythmHash: generateRhythmHash(log),
		attributes: (log.attributes || {}) as Record<string, unknown>,
		fullLogJson: raw,
	};
}

export function generateRhythmHash(log: {
	resource?: { attributes?: Record<string, unknown> };
	severityText?: string;
	body?: { stringValue?: string };
}): string {
	const service = String(
		log.resource?.attributes?.["service.name"] || "unknown",
	);
	const severity = String(log.severityText || "INFO");
	const body = String(log.body?.stringValue || "");

	const template = body
		.replace(
			/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/g,
			"*",
		)
		.replace(/\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/g, "*")
		.replace(/\b\d+\b/g, "*");

	const combined = `${service}:${severity}:${template}`;
	const hash = Bun.hash.xxHash64(combined);
	return hash.toString(16);
}

function generateId(): string {
	const hash = Bun.hash.xxHash64(`${Date.now()}-${Math.random()}`);
	return hash.toString(16);
}
