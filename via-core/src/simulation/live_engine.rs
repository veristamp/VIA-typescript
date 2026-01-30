//! Live Detection Engine
//!
//! Real-time anomaly detection integrated with simulation for live visualization
//! and interactive control.

use crate::engine::{AnomalyProfile, AnomalyResult};
use crate::simulation::live_types::*;
use crate::simulation::scenarios::{performance::*, security::*, traffic::*, Scenario};
use crate::simulation::types::LogRecord;
use std::collections::{HashMap, VecDeque};

/// Live detection engine that runs detectors on simulated events in real-time
pub struct LiveDetectionEngine {
    /// The SOTA detector profile
    profile: AnomalyProfile,

    /// Active scenarios
    scenarios: Vec<Box<dyn Scenario>>,

    /// Current simulation time
    current_time_ns: u64,
    start_time_ns: u64,

    /// Statistics
    total_events: u64,
    anomaly_count: u64,
    detector_hits: HashMap<String, u64>,

    /// Recent alerts (for dashboard)
    recent_alerts: VecDeque<Alert>,
    max_alerts: usize,

    /// Metric history for time-series charts
    metric_history: VecDeque<DataPoint>,
    max_history: usize,

    /// Current event rate
    target_rate: f64, // events per second
    current_rate: f64,

    /// Running state
    is_running: bool,
    is_paused: bool,

    /// Active scenario name
    current_scenario: String,

    /// Pending anomaly injection
    pending_anomaly: Option<(String, u64, u64)>, // (type, start_time, end_time)
}

impl LiveDetectionEngine {
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Self {
            profile: AnomalyProfile::default(),
            scenarios: Vec::new(),
            current_time_ns: now,
            start_time_ns: now,
            total_events: 0,
            anomaly_count: 0,
            detector_hits: HashMap::new(),
            recent_alerts: VecDeque::with_capacity(100),
            max_alerts: 100,
            metric_history: VecDeque::with_capacity(1000),
            max_history: 1000,
            target_rate: 100.0,
            current_rate: 0.0,
            is_running: false,
            is_paused: false,
            current_scenario: "none".to_string(),
            pending_anomaly: None,
        }
    }

    /// Start the simulation with a scenario
    pub fn start(&mut self, scenario_name: &str, intensity: f64) {
        self.scenarios.clear();
        self.profile.reset();
        self.total_events = 0;
        self.anomaly_count = 0;
        self.detector_hits.clear();
        self.recent_alerts.clear();
        self.metric_history.clear();

        self.start_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.current_time_ns = self.start_time_ns;

        // Add scenario based on name
        match scenario_name {
            "normal_traffic" => {
                self.scenarios
                    .push(Box::new(NormalTraffic::new(100.0 * intensity)));
            }
            "credential_stuffing" => {
                self.scenarios.push(Box::new(CredentialStuffing {
                    attack_rps: 500.0 * intensity,
                }));
            }
            "sql_injection" => {
                self.scenarios.push(Box::new(SqlInjection {
                    attack_rps: 50.0 * intensity,
                }));
            }
            "port_scan" => {
                self.scenarios.push(Box::new(PortScan {
                    source_ip: format!("45.33.22.{}", (intensity * 255.0) as u8),
                    scan_speed: 100.0 * intensity,
                }));
            }
            "memory_leak" => {
                self.scenarios.push(Box::new(MemoryLeak::new(
                    "payment-service",
                    10.0 * intensity,
                )));
            }
            "cpu_spike" => {
                self.scenarios.push(Box::new(CpuSpike::new(
                    "recommendation-engine",
                    intensity.clamp(0.0, 1.0),
                )));
            }
            _ => {
                // Default to normal traffic
                self.scenarios.push(Box::new(NormalTraffic::new(100.0)));
            }
        }

        self.current_scenario = scenario_name.to_string();
        self.is_running = true;
        self.is_paused = false;
    }

    /// Stop the simulation
    pub fn stop(&mut self) {
        self.is_running = false;
        self.scenarios.clear();
        self.current_scenario = "none".to_string();
    }

    /// Pause simulation
    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    /// Resume simulation
    pub fn resume(&mut self) {
        self.is_paused = false;
    }

    /// Set event generation rate
    pub fn set_rate(&mut self, events_per_second: f64) {
        self.target_rate = events_per_second.max(1.0).min(10000.0);
    }

    /// Inject an anomaly manually
    pub fn inject_anomaly(&mut self, anomaly_type: &str, duration_ms: u64) {
        let now = self.current_time_ns;
        let end = now + (duration_ms * 1_000_000);
        self.pending_anomaly = Some((anomaly_type.to_string(), now, end));
    }

    /// Reset all detector state
    pub fn reset_detectors(&mut self) {
        self.profile.reset();
        self.detector_hits.clear();
    }

    /// Process a tick and return live detection results
    pub fn tick(&mut self, delta_ns: u64) -> LiveOTelLog {
        if !self.is_running || self.is_paused {
            return LiveOTelLog {
                resourceLogs: vec![],
                timestamp: self.current_time_ns,
                simulationTimeMs: (self.current_time_ns - self.start_time_ns) / 1_000_000,
                totalEvents: self.total_events,
                anomalyCount: self.anomaly_count,
            };
        }

        let mut all_logs: Vec<LiveLogRecord> = Vec::new();

        // Generate events from scenarios - collect logs first to avoid borrow issues
        let mut pending_logs: Vec<LogRecord> = Vec::new();
        for scenario in &mut self.scenarios {
            let logs = scenario.tick(self.current_time_ns, delta_ns);
            pending_logs.extend(logs);
        }

        // Process collected logs
        for log in pending_logs {
            let live_log = self.process_log_with_detection(log);
            all_logs.push(live_log);
        }

        // Check for pending anomaly injection - clone to avoid borrow issues
        let pending = self.pending_anomaly.clone();
        if let Some((anomaly_type, start, end)) = pending {
            if self.current_time_ns >= start && self.current_time_ns <= end {
                let injected_logs = self.generate_injected_anomaly(&anomaly_type, delta_ns);
                all_logs.extend(injected_logs);
            }
            if self.current_time_ns > end {
                self.pending_anomaly = None;
            }
        }

        self.current_time_ns += delta_ns;
        self.current_rate = all_logs.len() as f64 / (delta_ns as f64 / 1_000_000_000.0);

        // Build response
        LiveOTelLog {
            resourceLogs: vec![LiveResourceLog {
                resource: Resource { attributes: vec![] },
                scopeLogs: vec![LiveScopeLog {
                    logRecords: all_logs,
                }],
            }],
            timestamp: self.current_time_ns,
            simulationTimeMs: (self.current_time_ns - self.start_time_ns) / 1_000_000,
            totalEvents: self.total_events,
            anomalyCount: self.anomaly_count,
        }
    }

    /// Process a single log record through the detection pipeline
    fn process_log_with_detection(&mut self, log: LogRecord) -> LiveLogRecord {
        self.total_events += 1;

        // Extract entity hash from trace ID or attributes
        let entity_hash = xxhash_rust::xxh3::xxh3_64(log.traceId.as_bytes());

        // Extract value for detection (latency, memory, etc.)
        let value = self.extract_metric_value(&log);

        // Run detection
        let timestamp = log
            .timeUnixNano
            .parse::<u64>()
            .unwrap_or(self.current_time_ns);
        let result = self
            .profile
            .process_with_hash(timestamp, entity_hash, value);

        // Convert to detection result
        let detection = self.convert_detection_result(&result);

        // Update statistics
        if result.is_anomaly {
            self.anomaly_count += 1;
            self.add_alert(&log, &detection);
        }

        // Track detector hits
        if result.anomaly_score > 0.5 {
            *self
                .detector_hits
                .entry(detection.detectorName.clone())
                .or_insert(0) += 1;
        }

        // Update metric history
        self.update_metric_history(timestamp, value, &result);

        LiveLogRecord {
            timeUnixNano: log.timeUnixNano,
            traceId: log.traceId,
            spanId: log.spanId,
            severityNumber: log.severityNumber,
            severityText: log.severityText,
            body: Self::any_value_to_string(&log.body),
            attributes: log
                .attributes
                .into_iter()
                .map(Self::convert_key_value)
                .collect(),
            detection,
            entityHash: entity_hash,
        }
    }

    /// Extract a numeric metric value from log attributes
    fn extract_metric_value(&self, log: &LogRecord) -> f64 {
        // Try various common metric attributes
        for attr in &log.attributes {
            match attr.key.as_str() {
                "http.duration_ms"
                | "process.memory.usage"
                | "process.cpu.utilization"
                | "net.host.port"
                | "http.status_code" => {
                    if let Some(val) = self.get_numeric_value(&attr.value) {
                        return val;
                    }
                }
                _ => {}
            }
        }

        // Default: use body length as a proxy metric
        Self::any_value_to_string(&log.body).len() as f64
    }

    fn get_numeric_value(&self, value: &crate::simulation::types::AnyValue) -> Option<f64> {
        use crate::simulation::types::AnyValue;
        match value {
            AnyValue::Int { intValue } => Some(*intValue as f64),
            AnyValue::Double { doubleValue } => Some(*doubleValue),
            _ => None,
        }
    }

    fn any_value_to_string(value: &crate::simulation::types::AnyValue) -> String {
        use crate::simulation::types::AnyValue;
        match value {
            AnyValue::String { stringValue } => stringValue.clone(),
            AnyValue::Int { intValue } => intValue.to_string(),
            AnyValue::Double { doubleValue } => doubleValue.to_string(),
            AnyValue::Bool { boolValue } => boolValue.to_string(),
        }
    }

    fn convert_key_value(kv: crate::simulation::types::KeyValue) -> KeyValue {
        KeyValue {
            key: kv.key,
            value: Self::convert_any_value(kv.value),
        }
    }

    fn convert_any_value(value: crate::simulation::types::AnyValue) -> AnyValue {
        use crate::simulation::types::AnyValue as TypesAnyValue;
        match value {
            TypesAnyValue::String { stringValue } => AnyValue::String { stringValue },
            TypesAnyValue::Int { intValue } => AnyValue::Int { intValue },
            TypesAnyValue::Double { doubleValue } => AnyValue::Double { doubleValue },
            TypesAnyValue::Bool { boolValue } => AnyValue::Bool { boolValue },
        }
    }

    /// Convert internal result to detection result
    fn convert_detection_result(&self, result: &AnomalyResult) -> DetectionResult {
        let signal_type = match result.signal_type {
            1 => "Volume/RPS",
            2 => "Distribution/Latency",
            3 => "Cardinality/Velocity",
            4 => "Burst/IAT",
            5 => "Spectral/FFT",
            6 => "ChangePoint/Trend",
            7 => "RRCF/Multivariate",
            8 => "MultiScale/Temporal",
            9 => "Behavioral/Fingerprint",
            10 => "Drift/Concept",
            _ => "Unknown",
        };

        let severity_text = match result.severity {
            0 => "Normal",
            1 => "Low",
            2 => "Medium",
            3 => "High",
            _ => "Critical",
        };

        DetectionResult {
            isAnomaly: result.is_anomaly,
            anomalyScore: result.anomaly_score,
            severity: result.severity,
            signalType: signal_type.to_string(),
            detectorName: format!("{}", signal_type),
            confidence: result.confidence,
            reason: if result.is_anomaly {
                format!("Anomaly detected: severity={}", severity_text)
            } else {
                "Normal".to_string()
            },
            expectedValue: result.expected,
            actualValue: result.actual,
            driftType: None,
        }
    }

    /// Generate injected anomaly logs
    fn generate_injected_anomaly(
        &mut self,
        anomaly_type: &str,
        _delta_ns: u64,
    ) -> Vec<LiveLogRecord> {
        let mut logs = Vec::new();
        let trace_id = format!("injected-{}", self.current_time_ns);
        let span_id = format!("span-{}", self.current_time_ns);

        match anomaly_type {
            "traffic_spike" => {
                // Generate burst of traffic
                for i in 0..100 {
                    let log = self.process_log_with_detection(LogRecord {
                        timeUnixNano: (self.current_time_ns + i * 1_000_000).to_string(),
                        traceId: format!("{}-{}", trace_id, i),
                        spanId: span_id.clone(),
                        severityNumber: 9,
                        severityText: "INFO".to_string(),
                        body: crate::simulation::types::AnyValue::string(format!(
                            "Burst request {}",
                            i
                        )),
                        attributes: vec![],
                    });
                    logs.push(log);
                }
            }
            "data_exfil" => {
                // Large payload logs
                let log = self.process_log_with_detection(LogRecord {
                    timeUnixNano: self.current_time_ns.to_string(),
                    traceId: trace_id.clone(),
                    spanId: span_id.clone(),
                    severityNumber: 13,
                    severityText: "WARN".to_string(),
                    body: crate::simulation::types::AnyValue::string(
                        "Large data transfer detected",
                    ),
                    attributes: vec![],
                });
                logs.push(log);
            }
            _ => {}
        }

        logs
    }

    /// Add an alert
    fn add_alert(&mut self, log: &LogRecord, detection: &DetectionResult) {
        let alert = Alert {
            timestamp: self.current_time_ns,
            severity: detection.severity,
            message: format!(
                "{}: {}",
                log.severityText,
                Self::any_value_to_string(&log.body)
            ),
            detector: detection.detectorName.clone(),
            score: detection.anomalyScore,
        };

        self.recent_alerts.push_back(alert);
        if self.recent_alerts.len() > self.max_alerts {
            self.recent_alerts.pop_front();
        }
    }

    /// Update metric history
    fn update_metric_history(&mut self, timestamp: u64, value: f64, result: &AnomalyResult) {
        let point = DataPoint {
            timestamp,
            value,
            predicted: Some(result.expected),
            is_anomaly: result.is_anomaly,
            anomaly_score: result.anomaly_score,
        };

        self.metric_history.push_back(point);
        if self.metric_history.len() > self.max_history {
            self.metric_history.pop_front();
        }
    }

    /// Get current simulation status
    pub fn get_status(&self) -> SimulationStatus {
        let detector_stats: Vec<DetectorStat> = self
            .detector_hits
            .iter()
            .map(|(name, hits)| DetectorStat {
                name: name.clone(),
                hits: *hits,
                avg_score: 0.0, // Would need to track average
                last_triggered: None,
            })
            .collect();

        SimulationStatus {
            isRunning: self.is_running,
            currentScenario: self.current_scenario.clone(),
            eventsGenerated: self.total_events,
            anomaliesDetected: self.anomaly_count,
            eventRate: self.current_rate,
            activeDetectors: self
                .profile
                .get_detector_stats()
                .iter()
                .map(|(name, _, _)| name.clone())
                .collect(),
            detectorStats: detector_stats,
            recentAlerts: self.recent_alerts.iter().cloned().collect(),
            uptimeMs: (self.current_time_ns - self.start_time_ns) / 1_000_000,
        }
    }

    /// Get dashboard state
    pub fn get_dashboard(&self) -> DashboardState {
        let detector_breakdown: Vec<(String, u64, f64)> = self
            .detector_hits
            .iter()
            .map(|(name, count)| (name.clone(), *count, 0.0))
            .collect();

        let mut metric_summaries = Vec::new();
        if !self.metric_history.is_empty() {
            let values: Vec<f64> = self.metric_history.iter().map(|p| p.value).collect();
            let anomalies: usize = self.metric_history.iter().filter(|p| p.is_anomaly).count();

            metric_summaries.push(MetricSummary {
                name: "Metric".to_string(),
                current: *values.last().unwrap_or(&0.0),
                avg: values.iter().sum::<f64>() / values.len() as f64,
                min: values.iter().cloned().fold(f64::INFINITY, f64::min),
                max: values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                p95: 0.0, // Would need sorting
                p99: 0.0,
                anomaly_rate: anomalies as f64 / self.metric_history.len() as f64,
                data_points: self.metric_history.iter().cloned().collect(),
            });
        }

        let scenario_info = get_available_scenarios()
            .into_iter()
            .find(|s| s.name == self.current_scenario);

        DashboardState {
            timestamp: self.current_time_ns,
            is_simulating: self.is_running && !self.is_paused,
            events_per_second: self.current_rate,
            total_events: self.total_events,
            active_anomalies: self.recent_alerts.len(),
            detector_breakdown,
            metric_summaries,
            recent_alerts: self.recent_alerts.iter().cloned().collect(),
            scenario_info,
        }
    }

    /// Process a command
    pub fn process_command(&mut self, command: SimulationCommand) -> Option<SimulationStatus> {
        match command {
            SimulationCommand::Start {
                scenario,
                intensity,
            } => {
                self.start(&scenario, intensity);
            }
            SimulationCommand::Stop => {
                self.stop();
            }
            SimulationCommand::Pause => {
                self.pause();
            }
            SimulationCommand::Resume => {
                self.resume();
            }
            SimulationCommand::InjectAnomaly {
                anomaly_type,
                duration_ms,
            } => {
                self.inject_anomaly(&anomaly_type, duration_ms);
            }
            SimulationCommand::SetRate { events_per_second } => {
                self.set_rate(events_per_second);
            }
            SimulationCommand::ResetDetectors => {
                self.reset_detectors();
            }
            _ => {}
        }

        Some(self.get_status())
    }

    /// Get detection events for streaming
    pub fn get_detection_events(&self, _limit: usize) -> Vec<DetectionEvent> {
        // This would return recent detection events for real-time streaming
        // For now, return empty (would be populated during tick)
        Vec::new()
    }

    /// Convert to JSON for FFI
    pub fn tick_json(&mut self, delta_ns: u64) -> String {
        let log = self.tick(delta_ns);
        serde_json::to_string(&log).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn status_json(&self) -> String {
        let status = self.get_status();
        serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn dashboard_json(&self) -> String {
        let dashboard = self.get_dashboard();
        serde_json::to_string(&dashboard).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for LiveDetectionEngine {
    fn default() -> Self {
        Self::new()
    }
}
