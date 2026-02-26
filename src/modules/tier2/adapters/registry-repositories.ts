import {
	activateTier1Policy,
	deletePatch,
	getActivePatches,
	getAllRules,
	getCurrentActivePolicy,
	getIncidentGraph,
	getLatestDeadLetters,
	getTier1PolicyByVersion,
	getTier2IncidentById,
	listTier1Policies,
	listTier2Decisions,
	listTier2Incidents,
	listTier2IncidentsForRun,
	patchAnomaly,
	saveDeadLetter,
	saveIncidentGraph,
	saveTier2Decision,
	upsertTier1PolicyArtifact,
	upsertTier2Incident,
} from "../../../db/registry";
import type {
	Tier2ControlRepository,
	Tier2DeadLetterRepository,
	Tier2IncidentGraphRepository,
	Tier2IncidentRepository,
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
