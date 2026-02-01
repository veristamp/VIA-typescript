//! Core Types for via-sim
//!
//! Minimal, unified types for OTel log simulation with ground truth tracking.
//! Types are co-located here as the single source of truth.

use serde::{Deserialize, Serialize};

// ============================================================================
// OTel Log Types (OTLP JSON format - camelCase for serialization)
// ============================================================================

/// Root OTel log batch structure
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[allow(non_snake_case)]
pub struct OTelLog {
    pub resourceLogs: Vec<ResourceLog>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[allow(non_snake_case)]
pub struct ResourceLog {
    pub resource: Resource,
    pub scopeLogs: Vec<ScopeLog>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Resource {
    pub attributes: Vec<KeyValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[allow(non_snake_case)]
pub struct ScopeLog {
    pub logRecords: Vec<LogRecord>,
}

/// Individual log record - primary unit of simulation
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LogRecord {
    pub timeUnixNano: String,
    pub traceId: String,
    pub spanId: String,
    pub severityNumber: u32,
    pub severityText: String,
    pub body: AnyValue,
    pub attributes: Vec<KeyValue>,
    /// Ground truth: is this log part of an injected anomaly?
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub isGroundTruthAnomaly: bool,
    /// Ground truth: anomaly ID if this log is part of an anomaly
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anomalyId: Option<String>,
}

impl Default for LogRecord {
    fn default() -> Self {
        Self {
            timeUnixNano: "0".to_string(),
            traceId: String::new(),
            spanId: String::new(),
            severityNumber: 9, // INFO
            severityText: "INFO".to_string(),
            body: AnyValue::string(""),
            attributes: Vec::new(),
            isGroundTruthAnomaly: false,
            anomalyId: None,
        }
    }
}

impl LogRecord {
    /// Get attribute value by key
    pub fn get_attribute(&self, key: &str) -> Option<&AnyValue> {
        self.attributes
            .iter()
            .find(|kv| kv.key == key)
            .map(|kv| &kv.value)
    }

    /// Get service name from attributes
    pub fn service_name(&self) -> Option<&str> {
        self.get_attribute("service.name").and_then(|v| v.as_str())
    }

    /// Extract numeric metric value for benchmarking
    pub fn metric_value(&self) -> f64 {
        for key in &[
            "http.duration_ms",
            "latency_ms",
            "process.memory.usage",
            "process.cpu.utilization",
            "http.status_code",
        ] {
            if let Some(v) = self.get_attribute(key) {
                if let Some(n) = v.as_f64() {
                    return n;
                }
            }
        }
        1.0
    }

    /// Mark this log as part of a ground truth anomaly
    pub fn mark_anomalous(&mut self, anomaly_id: String) {
        self.isGroundTruthAnomaly = true;
        self.anomalyId = Some(anomaly_id);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: AnyValue,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: AnyValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    pub fn string(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, AnyValue::string(value))
    }

    pub fn int(key: impl Into<String>, value: i64) -> Self {
        Self::new(key, AnyValue::int(value))
    }

    pub fn double(key: impl Into<String>, value: f64) -> Self {
        Self::new(key, AnyValue::double(value))
    }

    pub fn bool(key: impl Into<String>, value: bool) -> Self {
        Self::new(key, AnyValue::bool(value))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
#[allow(non_snake_case)]
pub enum AnyValue {
    String { stringValue: String },
    Int { intValue: i64 },
    Bool { boolValue: bool },
    Double { doubleValue: f64 },
}

impl AnyValue {
    pub fn string(s: impl Into<String>) -> Self {
        AnyValue::String {
            stringValue: s.into(),
        }
    }
    pub fn int(i: i64) -> Self {
        AnyValue::Int { intValue: i }
    }
    pub fn double(d: f64) -> Self {
        AnyValue::Double { doubleValue: d }
    }
    pub fn bool(b: bool) -> Self {
        AnyValue::Bool { boolValue: b }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            AnyValue::String { stringValue } => Some(stringValue),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            AnyValue::Int { intValue } => Some(*intValue),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AnyValue::Double { doubleValue } => Some(*doubleValue),
            AnyValue::Int { intValue } => Some(*intValue as f64),
            _ => None,
        }
    }
}

// ============================================================================
// Ground Truth for Benchmarking
// ============================================================================

/// Ground truth record for a single injected anomaly period
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroundTruth {
    /// Unique anomaly identifier
    pub anomaly_id: String,
    /// Start timestamp (nanoseconds since epoch)
    pub start_time_ns: u64,
    /// End timestamp (nanoseconds since epoch)
    pub end_time_ns: u64,
    /// Type of anomaly injected
    pub anomaly_type: String,
    /// Target services (empty = all services)
    pub target_services: Vec<String>,
    /// Number of logs generated during this anomaly
    pub log_count: u64,
}

impl GroundTruth {
    pub fn new(id: impl Into<String>, anomaly_type: impl Into<String>) -> Self {
        Self {
            anomaly_id: id.into(),
            start_time_ns: 0,
            end_time_ns: 0,
            anomaly_type: anomaly_type.into(),
            target_services: Vec::new(),
            log_count: 0,
        }
    }

    /// Check if a timestamp falls within this ground truth window
    pub fn contains_timestamp(&self, timestamp_ns: u64) -> bool {
        timestamp_ns >= self.start_time_ns && timestamp_ns <= self.end_time_ns
    }

    /// Check if a log matches this ground truth (time + service)
    pub fn matches_log(&self, log: &LogRecord) -> bool {
        let ts: u64 = log.timeUnixNano.parse().unwrap_or(0);
        if !self.contains_timestamp(ts) {
            return false;
        }
        if self.target_services.is_empty() {
            return true;
        }
        log.service_name()
            .map(|s| self.target_services.iter().any(|t| t == s))
            .unwrap_or(false)
    }
}

// ============================================================================
// Simulation Output
// ============================================================================

/// Simulation output batch with logs and ground truth
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SimulationBatch {
    /// OTel log batch
    pub logs: OTelLog,
    /// Ground truth for this batch (anomalies active during this time window)
    pub ground_truth: Vec<GroundTruth>,
    /// Simulation metadata
    pub metadata: BatchMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BatchMetadata {
    /// Current simulation time (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Time elapsed since simulation start (nanoseconds)
    pub elapsed_ns: u64,
    /// Total logs generated in this batch
    pub log_count: u64,
    /// Logs marked as ground truth anomalies
    pub anomaly_log_count: u64,
    /// Active scenarios
    pub active_scenarios: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_record_ground_truth() {
        let mut log = LogRecord::default();
        assert!(!log.isGroundTruthAnomaly);

        log.mark_anomalous("test-anomaly".to_string());
        assert!(log.isGroundTruthAnomaly);
        assert_eq!(log.anomalyId, Some("test-anomaly".to_string()));
    }

    #[test]
    fn test_ground_truth_matching() {
        let gt = GroundTruth {
            anomaly_id: "test".to_string(),
            start_time_ns: 1_000_000_000,
            end_time_ns: 2_000_000_000,
            anomaly_type: "Test".to_string(),
            target_services: vec![],
            log_count: 0,
        };

        let mut log = LogRecord::default();
        log.timeUnixNano = "1500000000".to_string();
        assert!(gt.matches_log(&log));

        log.timeUnixNano = "3000000000".to_string();
        assert!(!gt.matches_log(&log));
    }

    #[test]
    fn test_any_value_conversions() {
        let s = AnyValue::string("hello");
        assert_eq!(s.as_str(), Some("hello"));

        let i = AnyValue::int(42);
        assert_eq!(i.as_i64(), Some(42));
        assert_eq!(i.as_f64(), Some(42.0));

        let d = AnyValue::double(3.14);
        assert_eq!(d.as_f64(), Some(3.14));
    }
}
