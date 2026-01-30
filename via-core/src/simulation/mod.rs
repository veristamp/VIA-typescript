pub mod live_engine;
pub mod live_types;
pub mod scenarios;
pub mod types;

pub use live_engine::LiveDetectionEngine;
pub use live_types::get_available_scenarios;

use scenarios::Scenario;
use types::{OTelLog, Resource, ResourceLog, ScopeLog};

pub struct SimulationEngine {
    scenarios: Vec<Box<dyn Scenario>>,
    current_time_ns: u64,
}

impl SimulationEngine {
    pub fn new() -> Self {
        Self {
            scenarios: Vec::new(),
            current_time_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        }
    }

    pub fn add_scenario(&mut self, scenario: Box<dyn Scenario>) {
        self.scenarios.push(scenario);
    }

    pub fn clear_scenarios(&mut self) {
        self.scenarios.clear();
    }

    pub fn tick(&mut self, delta_ns: u64) -> OTelLog {
        let mut all_logs = Vec::new();

        for scenario in &mut self.scenarios {
            let logs = scenario.tick(self.current_time_ns, delta_ns);
            all_logs.extend(logs);
        }

        self.current_time_ns += delta_ns;

        OTelLog {
            resourceLogs: vec![ResourceLog {
                resource: Resource { attributes: vec![] },
                scopeLogs: vec![ScopeLog {
                    logRecords: all_logs,
                }],
            }],
        }
    }

    pub fn tick_json(&mut self, delta_ns: u64) -> String {
        let log = self.tick(delta_ns);
        serde_json::to_string(&log).unwrap_or_else(|_| "{}".to_string())
    }
}
