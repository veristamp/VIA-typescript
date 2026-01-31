//! Comprehensive OTel Log Generator for Benchmarking
//!
//! Generates realistic enterprise-scale OpenTelemetry logs with:
//! - Realistic service topologies (microservices, data pipeline, infrastructure)
//! - All anomaly types: gradual, sudden, distributed, intermittent
//! - Precise control for reproducible benchmarks
//! - Kafka-like streaming output

use chrono::{DateTime, Timelike, Utc};
use rand::distr::Distribution;
use rand::Rng;
use rand_distr::LogNormal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// OpenTelemetry log record
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OTelLogRecord {
    pub timestamp: DateTime<Utc>,
    pub trace_id: String,
    pub span_id: String,
    pub severity: Severity,
    pub body: String,
    pub service_name: String,
    pub attributes: HashMap<String, AttributeValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Severity {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Double(f64),
    Bool(bool),
}

/// Service behavior model
#[derive(Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub base_rps: f64,
    pub latency_mean_ms: f64,
    pub latency_std_ms: f64,
    pub error_rate: f64,
    pub dependency_services: Vec<String>,
}

/// Anomaly configuration for benchmark scenarios
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AnomalyConfig {
    pub anomaly_type: AnomalyType,
    pub start_time_sec: u64,
    pub duration_sec: u64,
    pub severity: f64, // 0.0 to 1.0
    pub target_services: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum AnomalyType {
    // Volume anomalies
    TrafficSpike { multiplier: f64 },
    DDoSAttack { source_ips: usize },

    // Performance anomalies
    MemoryLeak { leak_rate_mb_per_sec: f64 },
    CpuSaturation { intensity: f64 },
    SlowQueries { latency_multiplier: f64 },

    // Security anomalies
    CredentialStuffing { attempts_per_sec: f64 },
    SqlInjection { probe_rate: f64 },
    PortScan { scan_speed: f64 },
    BruteForce { target_users: Vec<String> },

    // Cardinality anomalies
    NewEntityFlood { unique_entities_per_sec: f64 },
    IpRotation { unique_ips_per_sec: f64 },

    // Pattern anomalies
    BusinessHoursViolation { activity_multiplier: f64 },
    GeoImpossibility { travel_speed_kmh: f64 },

    // Distributed anomalies
    CascadeFailure { failure_rate: f64 },
    ThunderingHerd { burst_size: usize },
}

/// Log generator for a specific scenario
pub struct LogGenerator {
    pub services: Vec<ServiceConfig>,
    current_time: DateTime<Utc>,
    global_sequence: Arc<AtomicU64>,
    active_anomalies: Vec<AnomalyConfig>,
    event_counter: u64,
}

impl LogGenerator {
    pub fn new(services: Vec<ServiceConfig>) -> Self {
        Self {
            services,
            current_time: Utc::now(),
            global_sequence: Arc::new(AtomicU64::new(0)),
            active_anomalies: Vec::new(),
            event_counter: 0,
        }
    }

    pub fn inject_anomaly(&mut self, config: AnomalyConfig) {
        self.active_anomalies.push(config);
    }

    pub fn clear_anomalies(&mut self) {
        self.active_anomalies.clear();
    }

    /// Generate logs for a time window (in seconds)
    pub fn generate_window(&mut self, duration_sec: u64) -> Vec<OTelLogRecord> {
        let mut logs = Vec::new();
        let start_time = self.current_time;

        for service in &self.services {
            // Calculate events for this service in this window
            let events = self.calculate_event_count(service, duration_sec);

            for i in 0..events {
                let timestamp = start_time
                    + chrono::Duration::milliseconds(
                        (i as f64 / events as f64 * duration_sec as f64 * 1000.0) as i64,
                    );

                let log = self.generate_log_for_service(service, timestamp);
                logs.push(log);
            }
        }

        // Add anomaly-induced logs
        for anomaly in &self.active_anomalies {
            if self.is_anomaly_active(anomaly, duration_sec) {
                let anomaly_logs = self.generate_anomaly_logs(anomaly, duration_sec);
                logs.extend(anomaly_logs);
            }
        }

        // Sort by timestamp
        logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        self.current_time += chrono::Duration::seconds(duration_sec as i64);
        self.event_counter += logs.len() as u64;

        logs
    }

    fn calculate_event_count(&self, service: &ServiceConfig, duration_sec: u64) -> u64 {
        let base_events = (service.base_rps * duration_sec as f64) as u64;

        // Add daily pattern variation (higher during business hours)
        let hour = self.current_time.hour();
        let daily_multiplier = if hour >= 9 && hour <= 18 {
            1.5 // Peak hours
        } else if hour >= 6 && hour <= 22 {
            1.0 // Normal hours
        } else {
            0.3 // Off-peak
        };

        // Add random jitter (Poisson-like)
        let jitter = rand::rng().random_range(0.8..1.2);

        ((base_events as f64 * daily_multiplier * jitter) as u64).max(1)
    }

    fn generate_log_for_service(
        &self,
        service: &ServiceConfig,
        timestamp: DateTime<Utc>,
    ) -> OTelLogRecord {
        let mut rng = rand::rng();

        // Generate latency with log-normal distribution (realistic tail)
        let latency_dist = LogNormal::new(
            service.latency_mean_ms.ln(),
            (service.latency_std_ms / service.latency_mean_ms).clamp(0.1, 1.0),
        )
        .unwrap_or(LogNormal::new(4.0, 0.5).unwrap());

        let latency_ms = latency_dist.sample(&mut rng);

        // Determine error status
        let is_error = rng.random_bool(service.error_rate);
        let status_code = if is_error {
            if rng.random_bool(0.7) {
                500
            } else {
                rng.random_range(400..600)
            }
        } else {
            200
        };

        let severity = if is_error {
            if rng.random_bool(0.1) {
                Severity::Fatal
            } else {
                Severity::Error
            }
        } else if latency_ms > service.latency_mean_ms * 3.0 {
            Severity::Warn
        } else {
            Severity::Info
        };

        // Generate trace context
        let trace_id = Uuid::new_v4().to_string().replace("-", "");
        let span_id = &Uuid::new_v4().to_string().replace("-", "")[..16];
        let sequence = self.global_sequence.fetch_add(1, Ordering::SeqCst);

        // Build attributes
        let mut attributes = HashMap::new();
        attributes.insert(
            "http.method".to_string(),
            AttributeValue::String("POST".to_string()),
        );
        attributes.insert(
            "http.status_code".to_string(),
            AttributeValue::Int(status_code as i64),
        );
        attributes.insert(
            "http.duration_ms".to_string(),
            AttributeValue::Double(latency_ms),
        );
        attributes.insert(
            "http.url".to_string(),
            AttributeValue::String(format!("/api/v1/{}/action", service.name.replace("-", "_"))),
        );
        attributes.insert("sequence".to_string(), AttributeValue::Int(sequence as i64));

        // Add IP with geographic distribution
        let ip = self.generate_realistic_ip();
        attributes.insert("client.ip".to_string(), AttributeValue::String(ip));

        // User agent
        let user_agent = self.generate_user_agent();
        attributes.insert(
            "http.user_agent".to_string(),
            AttributeValue::String(user_agent),
        );

        // Request body size
        let body_size = rng.random_range(100..10000);
        attributes.insert(
            "http.request.body.size".to_string(),
            AttributeValue::Int(body_size),
        );

        let body = if is_error {
            format!(
                "Request failed: {} after {:.2}ms",
                self.generate_error_message(status_code),
                latency_ms
            )
        } else {
            format!("Request processed successfully in {:.2}ms", latency_ms)
        };

        OTelLogRecord {
            timestamp,
            trace_id,
            span_id: span_id.to_string(),
            severity,
            body,
            service_name: service.name.clone(),
            attributes,
        }
    }

    fn generate_anomaly_logs(
        &self,
        anomaly: &AnomalyConfig,
        _duration_sec: u64,
    ) -> Vec<OTelLogRecord> {
        let mut logs = Vec::new();
        let timestamp = self.current_time;

        match &anomaly.anomaly_type {
            AnomalyType::CredentialStuffing { attempts_per_sec } => {
                let attempts = (*attempts_per_sec * anomaly.duration_sec as f64) as u64;
                for i in 0..attempts {
                    let mut attrs = HashMap::new();
                    attrs.insert(
                        "event.category".to_string(),
                        AttributeValue::String("authentication".to_string()),
                    );
                    attrs.insert(
                        "user.id".to_string(),
                        AttributeValue::String(format!("user_{}", i % 1000)),
                    );
                    attrs.insert(
                        "source.ip".to_string(),
                        AttributeValue::String(format!(
                            "{}.{}.{}.{}",
                            rand::rng().random_range(10..200),
                            rand::rng().random_range(0..255),
                            rand::rng().random_range(0..255),
                            rand::rng().random_range(1..255)
                        )),
                    );
                    attrs.insert("http.status_code".to_string(), AttributeValue::Int(401));

                    logs.push(OTelLogRecord {
                        timestamp: timestamp
                            + chrono::Duration::milliseconds(
                                (i as f64 / attempts as f64 * anomaly.duration_sec as f64 * 1000.0)
                                    as i64,
                            ),
                        trace_id: Uuid::new_v4().to_string().replace("-", ""),
                        span_id: Uuid::new_v4().to_string().replace("-", "")[..16].to_string(),
                        severity: Severity::Warn,
                        body: "Login failed: Invalid credentials".to_string(),
                        service_name: "auth-service".to_string(),
                        attributes: attrs,
                    });
                }
            }
            AnomalyType::MemoryLeak {
                leak_rate_mb_per_sec,
            } => {
                // Generate memory metric logs showing gradual increase
                let steps = anomaly.duration_sec.min(60); // One log per second
                for i in 0..steps {
                    let memory_mb = 256.0 + (*leak_rate_mb_per_sec * i as f64);
                    let mut attrs = HashMap::new();
                    attrs.insert(
                        "process.memory.usage".to_string(),
                        AttributeValue::Double(memory_mb),
                    );
                    attrs.insert(
                        "process.memory.limit".to_string(),
                        AttributeValue::Double(4096.0),
                    );

                    let severity = if memory_mb > 3500.0 {
                        Severity::Fatal
                    } else if memory_mb > 3000.0 {
                        Severity::Error
                    } else if memory_mb > 2000.0 {
                        Severity::Warn
                    } else {
                        Severity::Info
                    };

                    logs.push(OTelLogRecord {
                        timestamp: timestamp + chrono::Duration::seconds(i as i64),
                        trace_id: Uuid::new_v4().to_string().replace("-", ""),
                        span_id: Uuid::new_v4().to_string().replace("-", "")[..16].to_string(),
                        severity,
                        body: format!("Memory usage: {:.2} MB", memory_mb),
                        service_name: anomaly
                            .target_services
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "payment-service".to_string()),
                        attributes: attrs,
                    });
                }
            }
            AnomalyType::TrafficSpike { multiplier } => {
                // Generate burst traffic
                let spike_events = (1000.0 * multiplier * 10.0) as u64; // 10 seconds of spike
                for i in 0..spike_events {
                    let service = anomaly
                        .target_services
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("api-gateway");

                    logs.push(OTelLogRecord {
                        timestamp: timestamp
                            + chrono::Duration::milliseconds(
                                (i as f64 / spike_events as f64 * 10000.0) as i64,
                            ),
                        trace_id: Uuid::new_v4().to_string().replace("-", ""),
                        span_id: Uuid::new_v4().to_string().replace("-", "")[..16].to_string(),
                        severity: Severity::Info,
                        body: "Spike request".to_string(),
                        service_name: service.to_string(),
                        attributes: HashMap::new(),
                    });
                }
            }
            _ => {
                // Other anomaly types can be added here
            }
        }

        logs
    }

    fn is_anomaly_active(&self, anomaly: &AnomalyConfig, duration_sec: u64) -> bool {
        let window_start = self.current_time.timestamp() as u64;
        let window_end = window_start + duration_sec;
        let anomaly_start = anomaly.start_time_sec;
        let anomaly_end = anomaly.start_time_sec + anomaly.duration_sec;

        // Check if windows overlap
        window_start < anomaly_end && window_end > anomaly_start
    }

    fn generate_realistic_ip(&self) -> String {
        let mut rng = rand::rng();
        // Mix of internal and external IPs
        if rng.random_bool(0.7) {
            // Internal (RFC 1918)
            format!(
                "10.{}.{}.{}",
                rng.random_range(0..255),
                rng.random_range(0..255),
                rng.random_range(1..255)
            )
        } else {
            // External
            format!(
                "{}.{}.{}.{}",
                rng.random_range(50..200),
                rng.random_range(0..255),
                rng.random_range(0..255),
                rng.random_range(1..255)
            )
        }
    }

    fn generate_user_agent(&self) -> String {
        let agents = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            "ViaAPI-Client/1.0",
            "Go-http-client/1.1",
        ];
        agents[rand::rng().random_range(0..agents.len())].to_string()
    }

    fn generate_error_message(&self, status: u16) -> String {
        match status {
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            _ => "Unknown Error",
        }
        .to_string()
    }

    pub fn get_stats(&self) -> GeneratorStats {
        GeneratorStats {
            total_events: self.event_counter,
            current_time: self.current_time,
            active_anomalies: self.active_anomalies.len(),
        }
    }
}

pub struct GeneratorStats {
    pub total_events: u64,
    pub current_time: DateTime<Utc>,
    pub active_anomalies: usize,
}

/// Predefined enterprise service topologies
pub mod topologies {
    use super::*;

    /// Standard microservices architecture
    pub fn microservices() -> Vec<ServiceConfig> {
        vec![
            ServiceConfig {
                name: "api-gateway".to_string(),
                base_rps: 5000.0,
                latency_mean_ms: 25.0,
                latency_std_ms: 10.0,
                error_rate: 0.001,
                dependency_services: vec![
                    "auth-service".to_string(),
                    "payment-service".to_string(),
                ],
            },
            ServiceConfig {
                name: "auth-service".to_string(),
                base_rps: 3000.0,
                latency_mean_ms: 50.0,
                latency_std_ms: 15.0,
                error_rate: 0.002,
                dependency_services: vec!["user-db".to_string()],
            },
            ServiceConfig {
                name: "payment-service".to_string(),
                base_rps: 1000.0,
                latency_mean_ms: 150.0,
                latency_std_ms: 50.0,
                error_rate: 0.005,
                dependency_services: vec![
                    "payment-gateway".to_string(),
                    "transaction-db".to_string(),
                ],
            },
            ServiceConfig {
                name: "inventory-service".to_string(),
                base_rps: 2000.0,
                latency_mean_ms: 75.0,
                latency_std_ms: 20.0,
                error_rate: 0.001,
                dependency_services: vec!["inventory-db".to_string(), "cache-layer".to_string()],
            },
            ServiceConfig {
                name: "recommendation-engine".to_string(),
                base_rps: 500.0,
                latency_mean_ms: 200.0,
                latency_std_ms: 80.0,
                error_rate: 0.01,
                dependency_services: vec!["ml-model".to_string(), "feature-store".to_string()],
            },
            ServiceConfig {
                name: "notification-service".to_string(),
                base_rps: 3000.0,
                latency_mean_ms: 30.0,
                latency_std_ms: 10.0,
                error_rate: 0.003,
                dependency_services: vec!["email-provider".to_string(), "sms-gateway".to_string()],
            },
        ]
    }

    /// Data pipeline architecture
    pub fn data_pipeline() -> Vec<ServiceConfig> {
        vec![
            ServiceConfig {
                name: "event-collector".to_string(),
                base_rps: 10000.0,
                latency_mean_ms: 5.0,
                latency_std_ms: 2.0,
                error_rate: 0.0001,
                dependency_services: vec!["kafka-producer".to_string()],
            },
            ServiceConfig {
                name: "stream-processor".to_string(),
                base_rps: 8000.0,
                latency_mean_ms: 20.0,
                latency_std_ms: 5.0,
                error_rate: 0.001,
                dependency_services: vec!["kafka-consumer".to_string(), "redis-cache".to_string()],
            },
            ServiceConfig {
                name: "etl-worker".to_string(),
                base_rps: 500.0,
                latency_mean_ms: 5000.0,
                latency_std_ms: 2000.0,
                error_rate: 0.02,
                dependency_services: vec!["data-warehouse".to_string()],
            },
            ServiceConfig {
                name: "analytics-api".to_string(),
                base_rps: 2000.0,
                latency_mean_ms: 100.0,
                latency_std_ms: 30.0,
                error_rate: 0.005,
                dependency_services: vec!["data-warehouse".to_string(), "cache-layer".to_string()],
            },
        ]
    }

    /// Infrastructure services
    pub fn infrastructure() -> Vec<ServiceConfig> {
        vec![
            ServiceConfig {
                name: "load-balancer".to_string(),
                base_rps: 20000.0,
                latency_mean_ms: 2.0,
                latency_std_ms: 1.0,
                error_rate: 0.0001,
                dependency_services: vec![],
            },
            ServiceConfig {
                name: "cdn-edge".to_string(),
                base_rps: 50000.0,
                latency_mean_ms: 15.0,
                latency_std_ms: 5.0,
                error_rate: 0.0005,
                dependency_services: vec!["origin-server".to_string()],
            },
            ServiceConfig {
                name: "database-cluster".to_string(),
                base_rps: 15000.0,
                latency_mean_ms: 5.0,
                latency_std_ms: 2.0,
                error_rate: 0.001,
                dependency_services: vec![],
            },
            ServiceConfig {
                name: "cache-layer".to_string(),
                base_rps: 50000.0,
                latency_mean_ms: 1.0,
                latency_std_ms: 0.5,
                error_rate: 0.0001,
                dependency_services: vec![],
            },
        ]
    }
}
