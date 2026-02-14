import type { Tier2Incident } from "../db/schema";
import type { Tier1PolicyRule, Tier1PolicySnapshot } from "../types";

export interface CompiledPolicyArtifact {
	policyVersion: string;
	snapshot: Tier1PolicySnapshot;
	featureFlags: Record<string, unknown>;
}

export class PolicyCompilerService {
	private parseEntityHash(entityKey: string): string[] {
		if (!entityKey.startsWith("hash:")) {
			return [];
		}
		const hash = entityKey.slice("hash:".length).trim();
		return hash.length > 0 ? [hash] : [];
	}

	private ruleFromIncident(incident: Tier2Incident): Tier1PolicyRule | null {
		const confidence = Math.max(0, Math.min(1, incident.confidence / 100));
		if (confidence < 0.6) {
			return null;
		}

		const entityHashes = this.parseEntityHash(incident.entityKey);

		if (incident.status === "suppressed") {
			return {
				pattern_id: incident.incidentId,
				action: "suppress",
				entity_hashes: entityHashes,
				min_confidence: confidence,
				ttl_sec: 3600,
			};
		}

		if (incident.status === "escalated" || incident.status === "merged") {
			return {
				pattern_id: incident.incidentId,
				action: "boost",
				entity_hashes: entityHashes,
				min_confidence: confidence,
				score_scale: 1.1,
				confidence_scale: 1.05,
				ttl_sec: 1800,
			};
		}

		return null;
	}

	compile(
		incidents: Tier2Incident[],
		seedVersion?: string,
	): CompiledPolicyArtifact {
		const now = Math.floor(Date.now() / 1000);
		const policyVersion =
			seedVersion ||
			`policy-${now}-${Bun.hash.xxHash64(String(now)).toString(16).slice(0, 8)}`;

		const rules = incidents
			.map((incident) => this.ruleFromIncident(incident))
			.filter((rule): rule is Tier1PolicyRule => rule !== null);

		const snapshot: Tier1PolicySnapshot = {
			version: policyVersion,
			created_at_unix: now,
			rules,
			defaults: {
				score_scale: 1.0,
				confidence_scale: 1.0,
			},
		};

		return {
			policyVersion,
			snapshot,
			featureFlags: {
				enable_policy_runtime: true,
				rule_count: rules.length,
				compiled_at: now,
			},
		};
	}
}

