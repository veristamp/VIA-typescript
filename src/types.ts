import type {
	Tier1PolicyRule as ContractTier1PolicyRule,
	Tier1PolicySnapshot as ContractTier1PolicySnapshot,
} from "./modules/tier2/contracts/tier1-policy";

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

export interface CanonicalTier2Event {
	eventId: string;
	schemaVersion: number;
	entityHash: string;
	entityId: string;
	timestamp: number;
	score: number;
	severity: number;
	primaryDetector: number;
	detectorsFired: number;
	confidence: number;
	detectorScores: number[];
	attributes: Record<string, unknown>;
}

export type IncidentStatus = "new" | "suppressed" | "merged" | "escalated";

export interface IncidentCandidate {
	incidentId: string;
	memberPointIds: string[];
	reason: "temporal" | "semantic" | "trace";
	confidence: number;
	firstSeenTs: number;
	lastSeenTs: number;
	severityMax: number;
	scoreMax: number;
	entityKey: string;
	evidence: Record<string, unknown>;
}

export type PolicyStatus = "draft" | "active" | "rolled_back";

export type Tier1PolicyRule = ContractTier1PolicyRule;
export type Tier1PolicySnapshot = ContractTier1PolicySnapshot;
