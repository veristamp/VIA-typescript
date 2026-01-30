//! Enhanced Simulation Types with Live Detection Support
//!
//! This module extends the basic simulation types to support real-time
//! anomaly detection, streaming results, and interactive controls.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Real-time detection result attached to a log record
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct DetectionResult {
    pub isAnomaly: bool,
    pub anomalyScore: f64,
    pub severity: u8, // 0=Normal, 1=Low, 2=Medium, 3=High, 4=Critical
    pub signalType: String,
    pub detectorName: String,
    pub confidence: f64,
    pub reason: String,
    pub expectedValue: f64,
    pub actualValue: f64,
    pub driftType: Option<String>, // sudden, gradual, incremental
}

impl DetectionResult {
    pub fn normal() -> Self {
        Self {
            isAnomaly: false,
            anomalyScore: 0.0,
            severity: 0,
            signalType: "normal".to_string(),
            detectorName: "none".to_string(),
            confidence: 1.0,
            reason: "No anomaly detected".to_string(),
            expectedValue: 0.0,
            actualValue: 0.0,
            driftType: None,
        }
    }
}

/// Log record with detection results
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LiveLogRecord {
    pub timeUnixNano: String,
    pub traceId: String,
    pub spanId: String,
    pub severityNumber: u32,
    pub severityText: String,
    pub body: String,
    pub attributes: Vec<KeyValue>,
    pub detection: DetectionResult,
    pub entityHash: u64,
}

/// Enhanced OTel log with detection
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LiveOTelLog {
    pub resourceLogs: Vec<LiveResourceLog>,
    pub timestamp: u64,
    pub simulationTimeMs: u64,
    pub totalEvents: u64,
    pub anomalyCount: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LiveResourceLog {
    pub resource: Resource,
    pub scopeLogs: Vec<LiveScopeLog>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct LiveScopeLog {
    pub logRecords: Vec<LiveLogRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Resource {
    pub attributes: Vec<KeyValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: AnyValue,
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
}

/// Simulation control commands
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command")]
pub enum SimulationCommand {
    Start {
        scenario: String,
        intensity: f64,
    },
    Stop,
    Pause,
    Resume,
    InjectAnomaly {
        anomaly_type: String,
        duration_ms: u64,
    },
    SetRate {
        events_per_second: f64,
    },
    EnableDetector {
        detector_name: String,
    },
    DisableDetector {
        detector_name: String,
    },
    ResetDetectors,
    GetStatus,
}

/// Simulation status response
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct SimulationStatus {
    pub isRunning: bool,
    pub currentScenario: String,
    pub eventsGenerated: u64,
    pub anomaliesDetected: u64,
    pub eventRate: f64,
    pub activeDetectors: Vec<String>,
    pub detectorStats: Vec<DetectorStat>,
    pub recentAlerts: VecDeque<Alert>,
    pub uptimeMs: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectorStat {
    pub name: String,
    pub hits: u64,
    pub avg_score: f64,
    pub last_triggered: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alert {
    pub timestamp: u64,
    pub severity: u8,
    pub message: String,
    pub detector: String,
    pub score: f64,
}

/// Scenario configuration for dynamic loading
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScenarioConfig {
    pub name: String,
    pub description: String,
    pub params: Vec<ScenarioParam>,
    pub category: ScenarioCategory,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScenarioParam {
    pub name: String,
    pub param_type: ParamType,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ParamType {
    Float,
    Integer,
    Boolean,
    Percentage,
    Duration,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScenarioCategory {
    Security,
    Performance,
    Traffic,
    Custom,
}

/// Streaming detection event for real-time visualization
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectionEvent {
    pub timestamp: u64,
    pub entity_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub is_anomaly: bool,
    pub anomaly_score: f64,
    pub detector_signals: Vec<DetectorSignal>,
    pub service_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectorSignal {
    pub detector_name: String,
    pub signal_type: u8,
    pub score: f64,
    pub weight: f64,
    pub triggered: bool,
}

/// Time-series data point for charts
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DataPoint {
    pub timestamp: u64,
    pub value: f64,
    pub predicted: Option<f64>,
    pub is_anomaly: bool,
    pub anomaly_score: f64,
}

/// Metric aggregation for dashboards
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MetricSummary {
    pub name: String,
    pub current: f64,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub p95: f64,
    pub p99: f64,
    pub anomaly_rate: f64,
    pub data_points: Vec<DataPoint>,
}

/// Complete dashboard state
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DashboardState {
    pub timestamp: u64,
    pub is_simulating: bool,
    pub events_per_second: f64,
    pub total_events: u64,
    pub active_anomalies: usize,
    pub detector_breakdown: Vec<(String, u64, f64)>, // (name, count, avg_score)
    pub metric_summaries: Vec<MetricSummary>,
    pub recent_alerts: Vec<Alert>,
    pub scenario_info: Option<ScenarioConfig>,
}

/// Available scenarios registry
pub fn get_available_scenarios() -> Vec<ScenarioConfig> {
    vec![
        ScenarioConfig {
            name: "normal_traffic".to_string(),
            description: "Normal application traffic with occasional errors".to_string(),
            params: vec![ScenarioParam {
                name: "rps".to_string(),
                param_type: ParamType::Float,
                default: 100.0,
                min: 10.0,
                max: 10000.0,
                description: "Requests per second".to_string(),
            }],
            category: ScenarioCategory::Traffic,
        },
        ScenarioConfig {
            name: "credential_stuffing".to_string(),
            description: "Brute force attack with rotating IPs".to_string(),
            params: vec![ScenarioParam {
                name: "attack_rps".to_string(),
                param_type: ParamType::Float,
                default: 500.0,
                min: 50.0,
                max: 5000.0,
                description: "Attack requests per second".to_string(),
            }],
            category: ScenarioCategory::Security,
        },
        ScenarioConfig {
            name: "sql_injection".to_string(),
            description: "SQL injection probe attempts".to_string(),
            params: vec![ScenarioParam {
                name: "attack_rps".to_string(),
                param_type: ParamType::Float,
                default: 50.0,
                min: 10.0,
                max: 500.0,
                description: "SQL injection probes per second".to_string(),
            }],
            category: ScenarioCategory::Security,
        },
        ScenarioConfig {
            name: "port_scan".to_string(),
            description: "Network port scanning activity".to_string(),
            params: vec![
                ScenarioParam {
                    name: "scan_speed".to_string(),
                    param_type: ParamType::Float,
                    default: 100.0,
                    min: 10.0,
                    max: 1000.0,
                    description: "Ports scanned per second".to_string(),
                },
                ScenarioParam {
                    name: "source_ip".to_string(),
                    param_type: ParamType::Float,
                    default: 0.0,
                    min: 0.0,
                    max: 255.0,
                    description: "Source IP (first octet)".to_string(),
                },
            ],
            category: ScenarioCategory::Security,
        },
        ScenarioConfig {
            name: "memory_leak".to_string(),
            description: "Gradual memory leak leading to OOM crash".to_string(),
            params: vec![ScenarioParam {
                name: "leak_rate".to_string(),
                param_type: ParamType::Float,
                default: 10.0,
                min: 1.0,
                max: 100.0,
                description: "MB leaked per second".to_string(),
            }],
            category: ScenarioCategory::Performance,
        },
        ScenarioConfig {
            name: "cpu_spike".to_string(),
            description: "CPU saturation and thread pool exhaustion".to_string(),
            params: vec![ScenarioParam {
                name: "intensity".to_string(),
                param_type: ParamType::Percentage,
                default: 0.8,
                min: 0.1,
                max: 1.0,
                description: "CPU saturation probability".to_string(),
            }],
            category: ScenarioCategory::Performance,
        },
        ScenarioConfig {
            name: "distributed_attack".to_string(),
            description: "Coordinated attack from multiple sources".to_string(),
            params: vec![
                ScenarioParam {
                    name: "attackers".to_string(),
                    param_type: ParamType::Integer,
                    default: 10.0,
                    min: 5.0,
                    max: 100.0,
                    description: "Number of attacking IPs".to_string(),
                },
                ScenarioParam {
                    name: "attack_rps".to_string(),
                    param_type: ParamType::Float,
                    default: 200.0,
                    min: 50.0,
                    max: 2000.0,
                    description: "Total requests per second".to_string(),
                },
            ],
            category: ScenarioCategory::Security,
        },
        ScenarioConfig {
            name: "traffic_spike".to_string(),
            description: "Sudden burst in legitimate traffic".to_string(),
            params: vec![
                ScenarioParam {
                    name: "spike_multiplier".to_string(),
                    param_type: ParamType::Float,
                    default: 10.0,
                    min: 2.0,
                    max: 100.0,
                    description: "Traffic multiplier".to_string(),
                },
                ScenarioParam {
                    name: "duration_ms".to_string(),
                    param_type: ParamType::Duration,
                    default: 60000.0,
                    min: 1000.0,
                    max: 300000.0,
                    description: "Spike duration in ms".to_string(),
                },
            ],
            category: ScenarioCategory::Traffic,
        },
    ]
}
