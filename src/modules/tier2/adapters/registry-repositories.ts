import {
	activateTier1Policy,
	deletePatch,
	getActivePatches,
	getAllRules,
	getCurrentActivePolicy,
	getIncidentGraph,
	getLatestDeadLetters,
	getLatestEvaluationMetrics,
	getSchema,
	getTier1PolicyByVersion,
	getTier2IncidentById,
	listSchemas,
	listTier1Policies,
	listTier2Decisions,
	listTier2Incidents,
	listTier2IncidentsForRun,
	patchAnomaly,
	saveDeadLetter,
	saveEvaluationMetrics,
	saveIncidentGraph,
	saveSchema,
	saveTier2Decision,
	upsertTier1PolicyArtifact,
	upsertTier2Incident,
} from "../../../db/registry";
import type {
	Tier2ControlRepository,
	Tier2DeadLetterRepository,
	Tier2EvaluationRepository,
	Tier2IncidentGraphRepository,
	Tier2IncidentRepository,
	Tier2SchemaRepository,
} from "../ports/repositories";

export const tier2IncidentRepository: Tier2IncidentRepository = {
	upsertIncident: upsertTier2Incident,
	saveDecision: saveTier2Decision,
	getIncidentById: getTier2IncidentById,
	listIncidents: listTier2Incidents,
	listIncidentsForRun: listTier2IncidentsForRun,
	listDecisions: listTier2Decisions,
};

export const tier2ControlRepository: Tier2ControlRepository = {
	getActivePatches,
	patchAnomaly,
	deletePatch,
	getAllRules,
	listTier2Incidents,
	upsertTier1PolicyArtifact,
	activateTier1Policy,
	getCurrentActivePolicy,
	getTier1PolicyByVersion,
	listTier1Policies,
};

export const tier2DeadLetterRepository: Tier2DeadLetterRepository = {
	saveDeadLetter,
	getLatestDeadLetters,
};

export const tier2IncidentGraphRepository: Tier2IncidentGraphRepository = {
	saveIncidentGraph,
	getIncidentGraph,
};

export const tier2SchemaRepository: Tier2SchemaRepository = {
	getSchema,
	saveSchema,
	listSchemas,
};

export const tier2EvaluationRepository: Tier2EvaluationRepository = {
	saveEvaluationMetrics,
	getLatestEvaluationMetrics,
};
