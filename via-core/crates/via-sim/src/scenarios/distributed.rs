//! Distributed and Complex Anomaly Scenarios
//!
//! Advanced scenarios that simulate:
//! - Cascade failures across services
//! - DDoS attacks with multiple sources
//! - Data exfiltration patterns
//! - Business logic abuse

use crate::core::{AnyValue, KeyValue, LogRecord};
use crate::scenarios::Scenario;
use crate::scenarios::traffic::create_log;
use rand::prelude::*;
use uuid::Uuid;

// ============================================================================
// DDoS Attack Scenario
// ============================================================================

/// Distributed Denial of Service attack from multiple IPs
pub struct DDoSAttack {
    pub target_service: String,
    pub source_ip_count: usize,
    pub requests_per_ip: f64,
    source_ips: Vec<String>,
}

impl DDoSAttack {
    pub fn new(target_service: &str, source_ips: usize, requests_per_ip: f64) -> Self {
        let mut rng = rand::rng();
        let ips: Vec<String> = (0..source_ips)
            .map(|_| {
                format!(
                    "{}.{}.{}.{}",
                    rng.random_range(1..255),
                    rng.random_range(0..256),
                    rng.random_range(0..256),
                    rng.random_range(1..255)
                )
            })
            .collect();

        Self {
            target_service: target_service.to_string(),
            source_ip_count: source_ips,
            requests_per_ip,
            source_ips: ips,
        }
    }
}

impl Scenario for DDoSAttack {
    fn name(&self) -> &str {
        "DDoS Attack"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.requests_per_ip * self.source_ip_count as f64 * seconds).round() as u64;
        let mut logs = Vec::new();

        for i in 0..count {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();
            let source_ip = self.source_ips.choose(&mut rng).unwrap();

            // Rate limiting kicks in
            let (level, status, msg) = if rng.random_bool(0.7) {
                ("WARN", 429, "Rate limit exceeded")
            } else if rng.random_bool(0.3) {
                ("ERROR", 503, "Service unavailable")
            } else {
                ("INFO", 200, "Request processed")
            };

            logs.push(create_log(
                level,
                format!("{} from {}", msg, source_ip),
                &self.target_service,
                &trace_id,
                &span_id,
                current_time_ns + (i * 1_000_000),
                vec![
                    KeyValue {
                        key: "http.status_code".to_string(),
                        value: AnyValue::int(status),
                    },
                    KeyValue {
                        key: "net.peer.ip".to_string(),
                        value: AnyValue::string(source_ip.clone()),
                    },
                    KeyValue {
                        key: "http.method".to_string(),
                        value: AnyValue::string("GET"),
                    },
                    KeyValue {
                        key: "threat.type".to_string(),
                        value: AnyValue::string("ddos"),
                    },
                ],
            ));
        }
        logs
    }
}

// ============================================================================
// Cascade Failure Scenario
// ============================================================================

/// Cascade failure propagating through service dependencies
pub struct CascadeFailure {
    pub initial_service: String,
    pub failure_rate: f64,
    pub affected_services: Vec<String>,
    current_failure_depth: usize,
}

impl CascadeFailure {
    pub fn new(initial_service: &str, failure_rate: f64) -> Self {
        Self {
            initial_service: initial_service.to_string(),
            failure_rate,
            affected_services: vec![
                "auth-service".to_string(),
                "payment-service".to_string(),
                "api-gateway".to_string(),
                "inventory-service".to_string(),
                "recommendation-engine".to_string(),
            ],
            current_failure_depth: 0,
        }
    }
}

impl Scenario for CascadeFailure {
    fn name(&self) -> &str {
        "Cascade Failure"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let mut logs = Vec::new();

        // Increment cascade depth over time
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        if rng.random_bool(0.1 * seconds)
            && self.current_failure_depth < self.affected_services.len()
        {
            self.current_failure_depth += 1;
        }

        // Generate failure logs for affected services
        for i in 0..=self
            .current_failure_depth
            .min(self.affected_services.len() - 1)
        {
            let service = &self.affected_services[i];

            if rng.random_bool(self.failure_rate) {
                let trace_id = Uuid::new_v4().simple().to_string();
                let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

                let (level, error_type) = if i == 0 {
                    ("FATAL", "RootCauseError")
                } else {
                    ("ERROR", "DependencyFailure")
                };

                logs.push(create_log(
                    level,
                    format!(
                        "Service {} failed due to dependency failure (depth: {})",
                        service, i
                    ),
                    service,
                    &trace_id,
                    &span_id,
                    current_time_ns,
                    vec![
                        KeyValue {
                            key: "error.type".to_string(),
                            value: AnyValue::string(error_type),
                        },
                        KeyValue {
                            key: "cascade.depth".to_string(),
                            value: AnyValue::int(i as i64),
                        },
                        KeyValue {
                            key: "cascade.root".to_string(),
                            value: AnyValue::string(self.initial_service.clone()),
                        },
                        KeyValue {
                            key: "http.status_code".to_string(),
                            value: AnyValue::int(503),
                        },
                    ],
                ));
            }
        }
        logs
    }
}

// ============================================================================
// Data Exfiltration Scenario
// ============================================================================

/// Suspicious data exfiltration pattern
pub struct DataExfiltration {
    pub exfil_rate_mb_per_sec: f64,
    pub target_endpoint: String,
    total_exfiltrated_mb: f64,
}

impl DataExfiltration {
    pub fn new(rate_mb: f64, target: &str) -> Self {
        Self {
            exfil_rate_mb_per_sec: rate_mb,
            target_endpoint: target.to_string(),
            total_exfiltrated_mb: 0.0,
        }
    }
}

impl Scenario for DataExfiltration {
    fn name(&self) -> &str {
        "Data Exfiltration"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let data_mb = self.exfil_rate_mb_per_sec * seconds;
        self.total_exfiltrated_mb += data_mb;

        let mut logs = Vec::new();

        if rng.random_bool(0.3) {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            // Suspicious external IP
            let external_ip = format!(
                "{}.{}.{}.{}",
                rng.random_range(50..200),
                rng.random_range(0..256),
                rng.random_range(0..256),
                rng.random_range(1..255)
            );

            let payload_size = (data_mb * 1024.0 * 1024.0) as i64;

            logs.push(create_log(
                "WARN",
                format!(
                    "Large outbound data transfer: {:.2} MB to {}",
                    data_mb, self.target_endpoint
                ),
                "api-gateway",
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "http.response.body.size".to_string(),
                        value: AnyValue::int(payload_size),
                    },
                    KeyValue {
                        key: "net.peer.ip".to_string(),
                        value: AnyValue::string(external_ip),
                    },
                    KeyValue {
                        key: "http.url".to_string(),
                        value: AnyValue::string(self.target_endpoint.clone()),
                    },
                    KeyValue {
                        key: "data.total_exfiltrated_mb".to_string(),
                        value: AnyValue::double(self.total_exfiltrated_mb),
                    },
                    KeyValue {
                        key: "threat.category".to_string(),
                        value: AnyValue::string("data_exfiltration"),
                    },
                ],
            ));
        }
        logs
    }
}

// ============================================================================
// Slow Query Pattern
// ============================================================================

/// Database slow queries pattern
pub struct SlowQueries {
    pub service_name: String,
    pub latency_multiplier: f64,
    pub query_rate: f64,
}

impl SlowQueries {
    pub fn new(service: &str, latency_mult: f64, rate: f64) -> Self {
        Self {
            service_name: service.to_string(),
            latency_multiplier: latency_mult,
            query_rate: rate,
        }
    }
}

impl Scenario for SlowQueries {
    fn name(&self) -> &str {
        "Slow Queries"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.query_rate * seconds).round() as u64;
        let mut logs = Vec::new();

        let slow_queries = [
            "SELECT * FROM orders WHERE user_id IN (SELECT id FROM users)",
            "SELECT COUNT(*) FROM transactions GROUP BY day",
            "UPDATE products SET inventory = inventory - 1 WHERE id = ?",
        ];

        for _ in 0..count {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            let base_latency = rng.random_range(50.0..200.0);
            let slow_latency = base_latency * self.latency_multiplier;
            let query = slow_queries.choose(&mut rng).unwrap();

            let level = if slow_latency > 5000.0 {
                "ERROR"
            } else if slow_latency > 1000.0 {
                "WARN"
            } else {
                "INFO"
            };

            logs.push(create_log(
                level,
                format!("Query executed in {}ms", slow_latency as i64),
                &self.service_name,
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "db.statement".to_string(),
                        value: AnyValue::string(*query),
                    },
                    KeyValue {
                        key: "db.duration_ms".to_string(),
                        value: AnyValue::double(slow_latency),
                    },
                    KeyValue {
                        key: "db.type".to_string(),
                        value: AnyValue::string("postgresql"),
                    },
                ],
            ));
        }
        logs
    }
}

// ============================================================================
// Error Rate Spike
// ============================================================================

/// Sudden increase in error rate
pub struct ErrorRateSpike {
    pub service_name: String,
    pub error_rate: f64,
    pub request_rate: f64,
}

impl ErrorRateSpike {
    pub fn new(service: &str, error_rate: f64, request_rate: f64) -> Self {
        Self {
            service_name: service.to_string(),
            error_rate,
            request_rate,
        }
    }
}

impl Scenario for ErrorRateSpike {
    fn name(&self) -> &str {
        "Error Rate Spike"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.request_rate * seconds).round() as u64;
        let mut logs = Vec::new();

        let error_messages = [
            "NullPointerException at Handler.process()",
            "Connection refused: downstream service unavailable",
            "Timeout waiting for database response",
            "OutOfMemoryError: GC overhead limit exceeded",
            "SocketException: Connection reset by peer",
        ];

        for _ in 0..count {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            let is_error = rng.random_bool(self.error_rate);

            if is_error {
                let error_msg = error_messages.choose(&mut rng).unwrap();
                let status_code = *[500, 502, 503, 504].choose(&mut rng).unwrap();

                logs.push(create_log(
                    "ERROR",
                    error_msg.to_string(),
                    &self.service_name,
                    &trace_id,
                    &span_id,
                    current_time_ns,
                    vec![
                        KeyValue {
                            key: "http.status_code".to_string(),
                            value: AnyValue::int(status_code),
                        },
                        KeyValue {
                            key: "error.type".to_string(),
                            value: AnyValue::string("ServerError"),
                        },
                    ],
                ));
            }
        }
        logs
    }
}

// ============================================================================
// Traffic Spike
// ============================================================================

/// Sudden traffic spike (legitimate or attack)
pub struct TrafficSpike {
    pub target_service: String,
    pub multiplier: f64,
    pub base_rps: f64,
}

impl TrafficSpike {
    pub fn new(service: &str, multiplier: f64, base_rps: f64) -> Self {
        Self {
            target_service: service.to_string(),
            multiplier,
            base_rps,
        }
    }
}

impl Scenario for TrafficSpike {
    fn name(&self) -> &str {
        "Traffic Spike"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.base_rps * self.multiplier * seconds).round() as u64;
        let mut logs = Vec::new();

        for i in 0..count {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            // High latency due to load
            let latency = rng.random_range(100.0..500.0) * (1.0 + self.multiplier / 10.0);

            let (level, status) = if latency > 2000.0 {
                ("WARN", 504)
            } else if rng.random_bool(0.02) {
                ("ERROR", 500)
            } else {
                ("INFO", 200)
            };

            logs.push(create_log(
                level,
                format!("Request processed in {:.0}ms", latency),
                &self.target_service,
                &trace_id,
                &span_id,
                current_time_ns + (i * 1_000_000 / count.max(1)),
                vec![
                    KeyValue {
                        key: "http.status_code".to_string(),
                        value: AnyValue::int(status),
                    },
                    KeyValue {
                        key: "http.duration_ms".to_string(),
                        value: AnyValue::double(latency),
                    },
                ],
            ));
        }
        logs
    }
}
