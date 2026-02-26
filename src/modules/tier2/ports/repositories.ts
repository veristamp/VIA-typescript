import type {
	UpsertPolicyArtifactInput,
	UpsertTier2IncidentInput,
} from "../../../db/registry";
import type {
	IncidentGraph,
	Patch,
	Tier1PolicyArtifact,
	Tier2DeadLetter,
	Tier2Decision,
	Tier2Incident,
} from "../../../db/schema";

export interface Tier2IncidentRepository {
	upsertIncident(input: UpsertTier2IncidentInput): Promise<void>;
	saveDecision(
		incidentId: string,
		decision: string,
		reason: string,
		confidencePct: number,
		policyVersion: string,
	): Promise<void>;
	getIncidentById(incidentId: string): Promise<Tier2Incident | undefined>;
	listIncidents(limit: number): Promise<Tier2Incident[]>;
	listIncidentsForRun(runId: string, limit: number): Promise<Tier2Incident[]>;
	listDecisions(incidentId: string, limit: number): Promise<Tier2Decision[]>;
}

export interface Tier2ControlRepository {
	getActivePatches(): Promise<Patch[]>;
	patchAnomaly(rhythmHash: string, reason: string): Promise<void>;
	deletePatch(rhythmHash: string): Promise<void>;
	getAllRules(): Promise<Patch[]>;
	listTier2Incidents(limit: number): Promise<Tier2Incident[]>;
	upsertTier1PolicyArtifact(input: UpsertPolicyArtifactInput): Promise<void>;
	activateTier1Policy(policyVersion: string): Promise<void>;
	getCurrentActivePolicy(): Promise<Tier1PolicyArtifact | undefined>;
	getTier1PolicyByVersion(
		policyVersion: string,
	): Promise<Tier1PolicyArtifact | undefined>;
	listTier1Policies(limit: number): Promise<Tier1PolicyArtifact[]>;
}

export interface Tier2DeadLetterRepository {
	saveDeadLetter(
		eventId: string,
		reason: string,
		payload: Record<string, unknown>,
	): Promise<void>;
	getLatestDeadLetters(limit: number): Promise<Tier2DeadLetter[]>;
}

export interface Tier2IncidentGraphRepository {
	saveIncidentGraph(
		metaIncidentId: string,
		qdrantPointId: string,
		linkType: string,
		confidence?: number,
	): Promise<void>;
	getIncidentGraph(metaIncidentId: string): Promise<IncidentGraph[]>;
}
