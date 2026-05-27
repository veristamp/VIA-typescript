//! Tier-2 contract helpers shared by the Rust hot path.
//!
//! Tier-2 still owns clustering, persistence, and policy compilation. The
//! deterministic per-event conversions live here so the forwarder can emit
//! canonical values before crossing the Rust-to-TypeScript boundary.

use serde::{Deserialize, Serialize};

pub const TIER1_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentDecisionStatus {
    New,
    Merged,
    Escalated,
}

impl IncidentDecisionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Merged => "merged",
            Self::Escalated => "escalated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IncidentDecision {
    pub status: IncidentDecisionStatus,
    pub confidence: f64,
}

pub fn normalize_tier1_severity(raw_severity: f64, schema_version: u16) -> f64 {
    if !raw_severity.is_finite() || raw_severity <= 0.0 {
        return 0.0;
    }

    if schema_version == TIER1_SCHEMA_VERSION
        && raw_severity.fract() == 0.0
        && (0.0..=4.0).contains(&raw_severity)
    {
        return raw_severity / 4.0;
    }

    if raw_severity <= 1.0 {
        return raw_severity;
    }

    (raw_severity / 4.0).min(1.0)
}

pub fn normalize_to_unix_seconds(timestamp: u64) -> u64 {
    if timestamp > 1_000_000_000_000_000
        || (timestamp > 10_000_000_000 && timestamp <= 1_000_000_000_000)
    {
        timestamp / 1_000_000_000
    } else if timestamp > 1_000_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    }
}

pub fn resolve_incident_decision(
    severity_max: f64,
    score_max: f64,
    member_count: usize,
    confidence: f64,
) -> IncidentDecision {
    let confidence = confidence.clamp(0.0, 1.0);
    let status = if severity_max >= 0.5 || score_max >= 0.6 {
        IncidentDecisionStatus::Escalated
    } else if member_count >= 2 && confidence >= 0.3 {
        IncidentDecisionStatus::Merged
    } else {
        IncidentDecisionStatus::New
    };

    IncidentDecision { status, confidence }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_v1_severity_enum_to_tier2_scale() {
        assert_eq!(normalize_tier1_severity(0.0, TIER1_SCHEMA_VERSION), 0.0);
        assert_eq!(normalize_tier1_severity(1.0, TIER1_SCHEMA_VERSION), 0.25);
        assert_eq!(normalize_tier1_severity(2.0, TIER1_SCHEMA_VERSION), 0.5);
        assert_eq!(normalize_tier1_severity(3.0, TIER1_SCHEMA_VERSION), 0.75);
        assert_eq!(normalize_tier1_severity(4.0, TIER1_SCHEMA_VERSION), 1.0);
    }

    #[test]
    fn keeps_already_normalized_severity_idempotent() {
        assert_eq!(normalize_tier1_severity(0.25, TIER1_SCHEMA_VERSION), 0.25);
        assert_eq!(normalize_tier1_severity(0.9, TIER1_SCHEMA_VERSION), 0.9);
    }

    #[test]
    fn normalizes_timestamp_units_to_seconds() {
        assert_eq!(normalize_to_unix_seconds(1_738_000_000), 1_738_000_000);
        assert_eq!(normalize_to_unix_seconds(1_738_000_000_000), 1_738_000_000);
        assert_eq!(
            normalize_to_unix_seconds(1_738_000_000_000_000_000),
            1_738_000_000
        );
    }

    #[test]
    fn resolves_incident_thresholds_with_tier2_parity() {
        assert_eq!(
            resolve_incident_decision(0.91, 0.5, 1, 0.7).status,
            IncidentDecisionStatus::Escalated
        );
        assert_eq!(
            resolve_incident_decision(0.4, 0.96, 1, 0.7).status,
            IncidentDecisionStatus::Escalated
        );
        assert_eq!(
            resolve_incident_decision(0.4, 0.5, 3, 0.85).status,
            IncidentDecisionStatus::Merged
        );
        assert_eq!(
            resolve_incident_decision(0.4, 0.5, 1, 0.7).status,
            IncidentDecisionStatus::New
        );
    }

    #[test]
    fn serializes_decision_status_for_tier2_contract() {
        assert_eq!(IncidentDecisionStatus::New.as_str(), "new");
        assert_eq!(IncidentDecisionStatus::Merged.as_str(), "merged");
        assert_eq!(IncidentDecisionStatus::Escalated.as_str(), "escalated");
    }
}
