//! Unified Simulation Engine
//!
//! Real-time OTel log generation with controlled anomaly injection.
//! Outputs logs with ground truth for benchmarking - NO detection logic.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  SimulationEngine                       │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
//! │  │  Scenarios  │  │  Scheduler  │  │  Ground Truth   │ │
//! │  │  (plugins)  │──│  (timing)   │──│  (tracking)     │ │
//! │  └─────────────┘  └─────────────┘  └─────────────────┘ │
//! │         │                │                   │         │
//! │         └────────────────┼───────────────────┘         │
//! │                          ▼                              │
//! │              ┌──────────────────────┐                   │
//! │              │   SimulationBatch    │                   │
//! │              │  (logs + ground_truth)                   │
//! │              └──────────────────────┘                   │
//! └─────────────────────────────────────────────────────────┘
//! ```

use crate::core::{
    BatchMetadata, GroundTruth, LogRecord, OTelLog, Resource, ResourceLog, ScopeLog,
    SimulationBatch,
};
use crate::scenarios::{self, Scenario};
use std::collections::HashMap;

/// Unified simulation engine
pub struct SimulationEngine {
    /// Active scenarios generating logs
    scenarios: Vec<Box<dyn Scenario>>,

    /// Baseline scenario (always running)
    baseline: Option<Box<dyn Scenario>>,

    /// Scheduled anomaly scenarios (start_time_ns -> scenario)
    scheduled: Vec<ScheduledScenario>,

    /// Current simulation time (nanoseconds)
    current_time_ns: u64,

    /// Simulation start time (nanoseconds)
    start_time_ns: u64,

    /// Ground truth tracking
    ground_truth: GroundTruthTracker,

    /// Engine state
    state: EngineState,

    /// Statistics
    stats: EngineStats,
}

/// Scheduled scenario for future activation
struct ScheduledScenario {
    scenario: Box<dyn Scenario>,
    start_time_ns: u64,
    end_time_ns: u64,
    anomaly_id: String,
    activated: bool,
}

/// Engine running state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Stopped,
    Running,
    Paused,
}

/// Ground truth tracker
struct GroundTruthTracker {
    /// Active ground truth records
    active: HashMap<String, GroundTruth>,
    /// Completed ground truth records
    completed: Vec<GroundTruth>,
}

impl GroundTruthTracker {
    fn new() -> Self {
        Self {
            active: HashMap::new(),
            completed: Vec::new(),
        }
    }

    fn start_anomaly(&mut self, id: String, anomaly_type: String, start_ns: u64, end_ns: u64) {
        self.active.insert(
            id.clone(),
            GroundTruth {
                anomaly_id: id,
                start_time_ns: start_ns,
                end_time_ns: end_ns,
                anomaly_type,
                target_services: Vec::new(),
                log_count: 0,
            },
        );
    }

    fn record_log(&mut self, anomaly_id: &str) {
        if let Some(gt) = self.active.get_mut(anomaly_id) {
            gt.log_count += 1;
        }
    }

    fn finalize_anomaly(&mut self, id: &str, current_time_ns: u64) {
        if let Some(mut gt) = self.active.remove(id) {
            gt.end_time_ns = current_time_ns;
            self.completed.push(gt);
        }
    }

    fn get_current_ground_truth(&self) -> Vec<GroundTruth> {
        let mut all: Vec<GroundTruth> = self.active.values().cloned().collect();
        all.extend(self.completed.iter().cloned());
        all
    }

    fn reset(&mut self) {
        self.active.clear();
        self.completed.clear();
    }
}

/// Engine statistics
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub total_logs: u64,
    pub total_anomaly_logs: u64,
    pub tick_count: u64,
    pub scenarios_activated: u64,
    pub scenarios_completed: u64,
}

impl SimulationEngine {
    /// Create a new simulation engine
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Self {
            scenarios: Vec::new(),
            baseline: None,
            scheduled: Vec::new(),
            current_time_ns: now,
            start_time_ns: now,
            ground_truth: GroundTruthTracker::new(),
            state: EngineState::Stopped,
            stats: EngineStats::default(),
        }
    }

    /// Start the simulation with a baseline scenario
    pub fn start(&mut self, baseline_scenario: &str) {
        self.reset();

        // Set baseline scenario
        if let Some(scenario) = scenarios::create_scenario(baseline_scenario) {
            self.baseline = Some(scenario);
        } else {
            // Default to normal traffic
            self.baseline = scenarios::create_scenario("normal_traffic");
        }

        self.start_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.current_time_ns = self.start_time_ns;
        self.state = EngineState::Running;
    }

    /// Stop the simulation
    pub fn stop(&mut self) {
        self.state = EngineState::Stopped;
        self.baseline = None;
        self.scenarios.clear();
        self.scheduled.clear();
    }

    /// Pause the simulation
    pub fn pause(&mut self) {
        if self.state == EngineState::Running {
            self.state = EngineState::Paused;
        }
    }

    /// Resume the simulation
    pub fn resume(&mut self) {
        if self.state == EngineState::Paused || self.state == EngineState::Stopped {
            self.state = EngineState::Running;
        }
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.scenarios.clear();
        self.baseline = None;
        self.scheduled.clear();
        self.ground_truth.reset();
        self.stats = EngineStats::default();
    }

    /// Clear all active scenarios
    pub fn clear_scenarios(&mut self) {
        self.scenarios.clear();
    }

    /// Add an immediate scenario (starts now)
    pub fn add_scenario(&mut self, scenario: Box<dyn Scenario>) {
        self.scenarios.push(scenario);
    }

    /// Add a scenario by name
    pub fn add_scenario_by_name(&mut self, name: &str) -> bool {
        if let Some(scenario) = scenarios::create_scenario(name) {
            self.scenarios.push(scenario);
            true
        } else {
            false
        }
    }

    /// Schedule an anomaly scenario for later
    pub fn schedule_anomaly(
        &mut self,
        scenario_name: &str,
        start_offset_ns: u64,
        duration_ns: u64,
    ) -> Option<String> {
        let scenario = scenarios::create_scenario(scenario_name)?;
        let anomaly_id = format!("{}_{}", scenario_name, self.scheduled.len());

        let start_time_ns = self.current_time_ns + start_offset_ns;
        let end_time_ns = start_time_ns + duration_ns;

        self.scheduled.push(ScheduledScenario {
            scenario,
            start_time_ns,
            end_time_ns,
            anomaly_id: anomaly_id.clone(),
            activated: false,
        });

        Some(anomaly_id)
    }

    /// Inject an anomaly immediately (convenience method)
    pub fn inject_anomaly(&mut self, scenario_name: &str, duration_ms: u64) -> Option<String> {
        self.schedule_anomaly(scenario_name, 0, duration_ms * 1_000_000)
    }

    /// Advance simulation by delta_ns and return generated logs with ground truth
    pub fn tick(&mut self, delta_ns: u64) -> SimulationBatch {
        if self.state != EngineState::Running {
            return SimulationBatch::default();
        }

        let mut all_logs: Vec<LogRecord> = Vec::new();
        let mut active_scenarios: Vec<String> = Vec::new();

        // Generate logs from baseline
        if let Some(ref mut baseline) = self.baseline {
            let logs = baseline.tick(self.current_time_ns, delta_ns);
            active_scenarios.push(baseline.name().to_string());
            all_logs.extend(logs);
        }

        // Generate logs from active scenarios
        for scenario in &mut self.scenarios {
            let logs = scenario.tick(self.current_time_ns, delta_ns);
            active_scenarios.push(scenario.name().to_string());
            all_logs.extend(logs);
        }

        // Process scheduled scenarios
        let current = self.current_time_ns;
        let end_time = current + delta_ns;

        // Activate scheduled scenarios
        for scheduled in &mut self.scheduled {
            if !scheduled.activated && current >= scheduled.start_time_ns {
                scheduled.activated = true;
                self.stats.scenarios_activated += 1;

                // Start ground truth tracking
                self.ground_truth.start_anomaly(
                    scheduled.anomaly_id.clone(),
                    scheduled.scenario.name().to_string(),
                    scheduled.start_time_ns,
                    scheduled.end_time_ns,
                );
            }
        }

        // Generate logs from active scheduled scenarios
        let mut completed_indices: Vec<usize> = Vec::new();
        for (i, scheduled) in self.scheduled.iter_mut().enumerate() {
            if scheduled.activated && current < scheduled.end_time_ns {
                let mut logs = scheduled.scenario.tick(current, delta_ns);

                // Mark logs as ground truth anomalies
                for log in &mut logs {
                    log.mark_anomalous(scheduled.anomaly_id.clone());
                    self.ground_truth.record_log(&scheduled.anomaly_id);
                }

                active_scenarios.push(format!("{}(anomaly)", scheduled.scenario.name()));
                all_logs.extend(logs);
            } else if scheduled.activated && current >= scheduled.end_time_ns {
                // Scenario completed
                self.ground_truth
                    .finalize_anomaly(&scheduled.anomaly_id, current);
                completed_indices.push(i);
            }
        }

        // Remove completed scenarios
        for i in completed_indices.iter().rev() {
            self.scheduled.remove(*i);
            self.stats.scenarios_completed += 1;
        }

        // Update time
        self.current_time_ns = end_time;
        self.stats.tick_count += 1;

        // Count anomaly logs
        let anomaly_log_count = all_logs.iter().filter(|l| l.isGroundTruthAnomaly).count() as u64;

        self.stats.total_logs += all_logs.len() as u64;
        self.stats.total_anomaly_logs += anomaly_log_count;

        // Build output
        SimulationBatch {
            logs: OTelLog {
                resourceLogs: vec![ResourceLog {
                    resource: Resource { attributes: vec![] },
                    scopeLogs: vec![ScopeLog {
                        logRecords: all_logs,
                    }],
                }],
            },
            ground_truth: self.ground_truth.get_current_ground_truth(),
            metadata: BatchMetadata {
                timestamp_ns: self.current_time_ns,
                elapsed_ns: self.current_time_ns - self.start_time_ns,
                log_count: self.stats.total_logs,
                anomaly_log_count,
                active_scenarios,
            },
        }
    }

    /// Get engine state
    pub fn state(&self) -> EngineState {
        self.state
    }

    /// Get engine statistics
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    /// Get current simulation time
    pub fn current_time(&self) -> u64 {
        self.current_time_ns
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> u64 {
        self.current_time_ns - self.start_time_ns
    }

    /// Convenience: tick with milliseconds
    pub fn tick_ms(&mut self, delta_ms: u64) -> SimulationBatch {
        self.tick(delta_ms * 1_000_000)
    }

    /// Output as JSON string
    pub fn tick_json(&mut self, delta_ns: u64) -> String {
        let batch = self.tick(delta_ns);
        serde_json::to_string(&batch).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_lifecycle() {
        let mut engine = SimulationEngine::new();

        assert_eq!(engine.state(), EngineState::Stopped);

        engine.start("normal_traffic");
        assert_eq!(engine.state(), EngineState::Running);

        engine.pause();
        assert_eq!(engine.state(), EngineState::Paused);

        engine.resume();
        assert_eq!(engine.state(), EngineState::Running);

        engine.stop();
        assert_eq!(engine.state(), EngineState::Stopped);
    }

    #[test]
    fn test_basic_generation() {
        let mut engine = SimulationEngine::new();
        engine.start("normal_traffic");

        // Generate 100ms of logs
        let batch = engine.tick(100_000_000);

        assert!(!batch.logs.resourceLogs.is_empty());
        assert!(batch.metadata.log_count > 0);
    }

    #[test]
    fn test_anomaly_injection() {
        let mut engine = SimulationEngine::new();
        engine.start("normal_traffic");

        // Inject anomaly
        let anomaly_id = engine.inject_anomaly("memory_leak", 1000);
        assert!(anomaly_id.is_some());

        // Generate logs
        let batch = engine.tick(100_000_000);

        // Should have ground truth
        assert!(!batch.ground_truth.is_empty());
    }

    #[test]
    fn test_scheduled_anomaly() {
        let mut engine = SimulationEngine::new();
        engine.start("normal_traffic");

        // Schedule anomaly with 0 offset (starts immediately), lasting 1 second
        let anomaly_id = engine.schedule_anomaly("credential_stuffing", 0, 1_000_000_000);
        assert!(anomaly_id.is_some());

        // Tick 100ms - anomaly should be active
        let batch = engine.tick(100_000_000);

        // Ground truth should contain the active anomaly
        assert!(
            !batch.ground_truth.is_empty(),
            "Ground truth should contain the active anomaly"
        );

        // Should have anomaly logs from credential stuffing scenario
        // (credential stuffing generates ~50 logs/sec, so at 100ms we should have ~5)
        assert!(
            batch.metadata.anomaly_log_count > 0,
            "Should have generated anomaly logs"
        );
    }
}
