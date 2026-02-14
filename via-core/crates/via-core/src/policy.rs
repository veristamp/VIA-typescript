//! Tier-1 Runtime Policy Snapshot and Rule Evaluation
//!
//! Policies are compiled by Tier-2 and pushed to Tier-1 as immutable snapshots.
//! Tier-1 keeps a small in-memory history for fast rollback.
//!
//! Performance optimizations:
//! - Hash-indexed entity lookup: O(1) vs O(n) linear scan
//! - Detector-indexed rules for fast detector-specific matching
//! - Cached TTL expiry timestamps to avoid repeated computation

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_HISTORY_LIMIT: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    #[default]
    Suppress,
    Boost,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternRule {
    pub pattern_id: String,
    pub action: PolicyAction,
    #[serde(default)]
    pub entity_hashes: Vec<u64>,
    pub primary_detector: Option<u8>,
    pub min_confidence: Option<f64>,
    pub score_scale: Option<f64>,
    pub confidence_scale: Option<f64>,
    pub ttl_sec: u64,
    #[serde(default)]
    pub detector_priors: Option<Vec<DetectorPriorAdjustment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorPriorAdjustment {
    pub detector_id: u8,
    pub alpha_delta: f64,
    pub beta_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDefaults {
    pub score_scale: f64,
    pub confidence_scale: f64,
}

impl Default for PolicyDefaults {
    fn default() -> Self {
        Self {
            score_scale: 1.0,
            confidence_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySnapshot {
    pub version: String,
    pub created_at_unix: u64,
    #[serde(default)]
    pub rules: Vec<PatternRule>,
    #[serde(default)]
    pub defaults: PolicyDefaults,
    #[serde(default)]
    pub canary_percent: f64,
    #[serde(default)]
    pub fallback_version: Option<String>,
}

impl Default for PolicySnapshot {
    fn default() -> Self {
        Self {
            version: "policy-default".to_string(),
            created_at_unix: 0,
            rules: Vec::new(),
            defaults: PolicyDefaults::default(),
            canary_percent: 100.0,
            fallback_version: None,
        }
    }
}

#[derive(Debug, Clone)]
struct IndexedRule {
    rule: PatternRule,
    expires_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IndexedPolicySnapshot {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    created_at_unix: u64,
    defaults: PolicyDefaults,
    entity_index: HashMap<u64, Vec<usize>>,
    detector_index: HashMap<u8, Vec<usize>>,
    wildcard_rules: Vec<usize>,
    rules: Vec<IndexedRule>,
    #[allow(dead_code)]
    cached_now: u64,
}

impl IndexedPolicySnapshot {
    pub fn from_snapshot(snapshot: &PolicySnapshot) -> Self {
        let now = now_unix();
        let mut entity_index: HashMap<u64, Vec<usize>> = HashMap::new();
        let mut detector_index: HashMap<u8, Vec<usize>> = HashMap::new();
        let mut wildcard_rules = Vec::new();

        let indexed_rules: Vec<IndexedRule> = snapshot
            .rules
            .iter()
            .enumerate()
            .map(|(idx, rule)| {
                if rule.entity_hashes.is_empty() && rule.primary_detector.is_none() {
                    wildcard_rules.push(idx);
                } else {
                    for &hash in &rule.entity_hashes {
                        entity_index.entry(hash).or_default().push(idx);
                    }
                    if let Some(det) = rule.primary_detector {
                        detector_index.entry(det).or_default().push(idx);
                    }
                }

                let expires_at = if rule.ttl_sec > 0 && snapshot.created_at_unix > 0 {
                    Some(snapshot.created_at_unix.saturating_add(rule.ttl_sec))
                } else {
                    None
                };

                IndexedRule {
                    rule: rule.clone(),
                    expires_at,
                }
            })
            .collect();

        Self {
            version: snapshot.version.clone(),
            created_at_unix: snapshot.created_at_unix,
            defaults: snapshot.defaults.clone(),
            entity_index,
            detector_index,
            wildcard_rules,
            rules: indexed_rules,
            cached_now: now,
        }
    }

    pub fn evaluate(
        &self,
        entity_hash: u64,
        primary_detector: u8,
        confidence: f64,
        now: u64,
    ) -> PolicyEffect {
        let mut effect = PolicyEffect::neutral();
        effect.score_scale = self.defaults.score_scale.max(0.0);
        effect.confidence_scale = self.defaults.confidence_scale.max(0.0);

        let mut checked_indices: smallvec::SmallVec<[usize; 16]> = smallvec::SmallVec::new();

        if let Some(indices) = self.entity_index.get(&entity_hash) {
            for &idx in indices {
                checked_indices.push(idx);
                if let Some(r) = self.apply_rule(idx, primary_detector, confidence, now) {
                    effect = r;
                }
            }
        }

        if let Some(indices) = self.detector_index.get(&primary_detector) {
            for &idx in indices {
                if checked_indices.contains(&idx) {
                    continue;
                }
                checked_indices.push(idx);
                if let Some(r) = self.apply_rule(idx, primary_detector, confidence, now) {
                    effect = r;
                }
            }
        }

        for &idx in &self.wildcard_rules {
            if checked_indices.contains(&idx) {
                continue;
            }
            if let Some(r) = self.apply_rule(idx, primary_detector, confidence, now) {
                effect = r;
            }
        }

        effect
    }

    fn apply_rule(
        &self,
        idx: usize,
        primary_detector: u8,
        confidence: f64,
        now: u64,
    ) -> Option<PolicyEffect> {
        let indexed = self.rules.get(idx)?;
        let rule = &indexed.rule;

        if let Some(det) = rule.primary_detector {
            if det != primary_detector {
                return None;
            }
        }

        if let Some(min_conf) = rule.min_confidence {
            if confidence < min_conf {
                return None;
            }
        }

        if let Some(expires) = indexed.expires_at {
            if now > expires {
                return None;
            }
        }

        let mut effect = PolicyEffect::neutral();
        match rule.action {
            PolicyAction::Suppress => {
                effect.suppress = true;
            }
            PolicyAction::Boost => {
                effect.score_scale = rule.score_scale.unwrap_or(1.0).max(0.0);
                effect.confidence_scale = rule.confidence_scale.unwrap_or(1.0).max(0.0);
            }
        }

        Some(effect)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PolicyEffect {
    pub suppress: bool,
    pub score_scale: f64,
    pub confidence_scale: f64,
}

impl PolicyEffect {
    pub fn neutral() -> Self {
        Self {
            suppress: false,
            score_scale: 1.0,
            confidence_scale: 1.0,
        }
    }
}

pub struct PolicyRuntime {
    active: RwLock<PolicySnapshot>,
    indexed: RwLock<Option<IndexedPolicySnapshot>>,
    history: RwLock<Vec<PolicySnapshot>>,
    history_limit: usize,
}

impl PolicyRuntime {
    pub fn new() -> Self {
        Self {
            active: RwLock::new(PolicySnapshot::default()),
            indexed: RwLock::new(None),
            history: RwLock::new(Vec::new()),
            history_limit: DEFAULT_HISTORY_LIMIT,
        }
    }

    pub fn current_snapshot(&self) -> PolicySnapshot {
        self.active.read().unwrap().clone()
    }

    pub fn current_version(&self) -> String {
        self.active.read().unwrap().version.clone()
    }

    pub fn install_snapshot(&self, snapshot: PolicySnapshot) {
        let mut active = self.active.write().unwrap();
        let previous = active.clone();
        let mut history = self.history.write().unwrap();
        history.push(previous);
        if history.len() > self.history_limit {
            let drain_count = history.len() - self.history_limit;
            history.drain(0..drain_count);
        }

        let indexed = IndexedPolicySnapshot::from_snapshot(&snapshot);
        *self.indexed.write().unwrap() = Some(indexed);
        *active = snapshot;
    }

    pub fn rollback_to_version(&self, version: &str) -> bool {
        let candidate = {
            let history = self.history.read().unwrap();
            history.iter().rev().find(|p| p.version == version).cloned()
        };

        if let Some(snapshot) = candidate {
            self.install_snapshot(snapshot);
            true
        } else {
            false
        }
    }

    pub fn evaluate(
        &self,
        entity_hash: u64,
        primary_detector: u8,
        confidence: f64,
    ) -> PolicyEffect {
        let now = now_unix();

        if let Some(indexed) = self.indexed.read().unwrap().as_ref() {
            return indexed.evaluate(entity_hash, primary_detector, confidence, now);
        }

        let snapshot = self.active.read().unwrap();
        let mut effect = PolicyEffect::neutral();
        effect.score_scale = snapshot.defaults.score_scale.max(0.0);
        effect.confidence_scale = snapshot.defaults.confidence_scale.max(0.0);

        for rule in &snapshot.rules {
            let has_entity_filter = !rule.entity_hashes.is_empty();
            let entity_match = !has_entity_filter || rule.entity_hashes.contains(&entity_hash);
            if !entity_match {
                continue;
            }

            if let Some(detector) = rule.primary_detector {
                if detector != primary_detector {
                    continue;
                }
            }

            if let Some(min_confidence) = rule.min_confidence {
                if confidence < min_confidence {
                    continue;
                }
            }

            let expires_at = snapshot.created_at_unix.saturating_add(rule.ttl_sec);
            if rule.ttl_sec > 0 && snapshot.created_at_unix > 0 && now > expires_at {
                continue;
            }

            match rule.action {
                PolicyAction::Suppress => {
                    effect.suppress = true;
                }
                PolicyAction::Boost => {
                    effect.score_scale *= rule.score_scale.unwrap_or(1.0).max(0.0);
                    effect.confidence_scale *= rule.confidence_scale.unwrap_or(1.0).max(0.0);
                }
            }
        }

        effect
    }
}

impl Default for PolicyRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub static GLOBAL_POLICY_RUNTIME: Lazy<PolicyRuntime> = Lazy::new(PolicyRuntime::new);

pub fn runtime() -> &'static PolicyRuntime {
    &GLOBAL_POLICY_RUNTIME
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_with_rule(rule: PatternRule) -> PolicySnapshot {
        PolicySnapshot {
            version: "policy-test-v1".to_string(),
            created_at_unix: now_unix(),
            rules: vec![rule],
            defaults: PolicyDefaults::default(),
            ..Default::default()
        }
    }

    #[test]
    fn test_boost_rule_matching() {
        let runtime = PolicyRuntime::new();
        runtime.install_snapshot(snapshot_with_rule(PatternRule {
            pattern_id: "r1".to_string(),
            action: PolicyAction::Boost,
            entity_hashes: vec![42],
            primary_detector: Some(3),
            min_confidence: Some(0.7),
            score_scale: Some(1.2),
            confidence_scale: Some(1.1),
            ttl_sec: 600,
            ..Default::default()
        }));

        let effect = runtime.evaluate(42, 3, 0.75);
        assert!(!effect.suppress);
        assert!(effect.score_scale > 1.0);
        assert!(effect.confidence_scale > 1.0);
    }

    #[test]
    fn test_suppress_rule_matching() {
        let runtime = PolicyRuntime::new();
        runtime.install_snapshot(snapshot_with_rule(PatternRule {
            pattern_id: "r2".to_string(),
            action: PolicyAction::Suppress,
            entity_hashes: vec![99],
            ttl_sec: 600,
            ..Default::default()
        }));

        let effect = runtime.evaluate(99, 1, 0.2);
        assert!(effect.suppress);
    }

    #[test]
    fn test_expired_rule_is_ignored() {
        let runtime = PolicyRuntime::new();
        runtime.install_snapshot(PolicySnapshot {
            version: "policy-expired".to_string(),
            created_at_unix: now_unix().saturating_sub(1000),
            rules: vec![PatternRule {
                pattern_id: "r3".to_string(),
                action: PolicyAction::Suppress,
                entity_hashes: vec![7],
                ttl_sec: 1,
                ..Default::default()
            }],
            ..Default::default()
        });

        let effect = runtime.evaluate(7, 1, 0.9);
        assert!(!effect.suppress);
    }

    #[test]
    fn test_rollback_to_previous_version() {
        let runtime = PolicyRuntime::new();
        runtime.install_snapshot(PolicySnapshot {
            version: "v1".to_string(),
            created_at_unix: now_unix(),
            ..Default::default()
        });
        runtime.install_snapshot(PolicySnapshot {
            version: "v2".to_string(),
            created_at_unix: now_unix(),
            ..Default::default()
        });

        assert!(runtime.rollback_to_version("v1"));
        assert_eq!(runtime.current_version(), "v1");
    }
}
