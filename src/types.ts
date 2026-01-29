export interface LogRecord {
	id: string;
	timestamp: number;
	service: string;
	severity: string;
	body: string;
	rhythmHash: string;
	attributes: Record<string, unknown>;
	fullLogJson?: unknown;
}

export interface BatchSummary {
	timestamp: number;
	totalLogs: number;
	groups: Array<{
		rhythmHash: string;
		service: string;
		count: number;
		uniqueIds: string[];
	}>;
}
