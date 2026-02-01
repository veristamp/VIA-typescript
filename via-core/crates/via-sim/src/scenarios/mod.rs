//! Simulation Scenarios
//!
//! Configurable scenarios for generating realistic anomaly patterns:
//! - **traffic**: Normal and spike traffic patterns
//! - **security**: Attack patterns (credential stuffing, SQL injection, port scan)
//! - **performance**: Resource issues (memory leak, CPU spike, slow queries)
//! - **distributed**: Complex patterns (cascade failure, DDoS, data exfiltration)

pub mod distributed;
pub mod performance;
pub mod security;
pub mod traffic;

use crate::core::LogRecord;

/// Trait for simulation scenarios
///
/// Scenarios generate log records based on time progression.
/// The `tick` method is called with the current simulation time
/// and time delta since last tick.
pub trait Scenario: Send {
    /// Human-readable name of the scenario
    fn name(&self) -> &str;

    /// Generate log records for this time step
    ///
    /// # Arguments
    /// * `current_time_ns` - Current simulation time in nanoseconds since epoch
    /// * `delta_ns` - Time elapsed since last tick in nanoseconds
    ///
    /// # Returns
    /// Vector of log records generated during this time step
    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord>;
}

// Re-export common scenarios for convenience
pub use distributed::{
    CascadeFailure, DDoSAttack, DataExfiltration, ErrorRateSpike, SlowQueries, TrafficSpike,
};
pub use performance::{CpuSpike, InfiniteLoop, MemoryLeak};
pub use security::{CredentialStuffing, PortScan, SqlInjection};
pub use traffic::NormalTraffic;

/// Create a scenario by name with default parameters
pub fn create_scenario(name: &str) -> Option<Box<dyn Scenario>> {
    match name.to_lowercase().as_str() {
        "normal_traffic" | "normal" => Some(Box::new(NormalTraffic::new(100.0))),
        "credential_stuffing" | "brute_force" => {
            Some(Box::new(CredentialStuffing { attack_rps: 50.0 }))
        }
        "sql_injection" | "sqli" => Some(Box::new(SqlInjection { attack_rps: 10.0 })),
        "port_scan" => Some(Box::new(PortScan {
            source_ip: "192.168.1.100".to_string(),
            scan_speed: 100.0,
        })),
        "memory_leak" => Some(Box::new(MemoryLeak::new("payment-service", 10.0))),
        "cpu_spike" => Some(Box::new(CpuSpike::new("stream-processor", 0.8))),
        "infinite_loop" | "stack_overflow" => Some(Box::new(InfiniteLoop {
            service_name: "recommendation-engine".to_string(),
        })),
        "ddos" | "ddos_attack" => Some(Box::new(DDoSAttack::new("api-gateway", 100, 10.0))),
        "cascade_failure" | "cascade" => Some(Box::new(CascadeFailure::new("auth-service", 0.3))),
        "data_exfiltration" | "exfil" => Some(Box::new(DataExfiltration::new(
            5.0,
            "external-collector.evil.com",
        ))),
        "slow_queries" => Some(Box::new(SlowQueries::new("inventory-service", 5.0, 10.0))),
        "error_spike" => Some(Box::new(ErrorRateSpike::new("payment-service", 0.5, 50.0))),
        "traffic_spike" => Some(Box::new(TrafficSpike::new("api-gateway", 10.0, 100.0))),
        _ => None,
    }
}

/// List all available scenarios
pub fn list_scenarios() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "normal_traffic",
            "Normal baseline traffic with realistic patterns",
        ),
        (
            "credential_stuffing",
            "Brute force login attempts from multiple IPs",
        ),
        ("sql_injection", "SQL injection probe attacks"),
        ("port_scan", "Network port scanning activity"),
        ("memory_leak", "Gradual memory consumption leading to OOM"),
        ("cpu_spike", "High CPU utilization causing timeouts"),
        ("infinite_loop", "Stack overflow from infinite recursion"),
        ("ddos", "Distributed denial of service attack"),
        (
            "cascade_failure",
            "Service failure propagating through dependencies",
        ),
        ("data_exfiltration", "Suspicious large data transfers"),
        ("slow_queries", "Database performance degradation"),
        ("error_spike", "Sudden increase in error rates"),
        ("traffic_spike", "Sudden traffic burst"),
    ]
}
