export interface Tier1PolicyRule {
	pattern_id: string;
	action: "suppress" | "boost";
	entity_hashes?: number[];
	primary_detector?: number;
	min_confidence?: number;
	score_scale?: number;
	confidence_scale?: number;
	ttl_sec: number;
}

export interface Tier1PolicySnapshot {
	version: string;
	created_at_unix: number;
	rules: Tier1PolicyRule[];
	defaults: {
		score_scale: number;
		confidence_scale: number;
	};
}
