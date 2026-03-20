use crate::core::{AnyValue, KeyValue, LogRecord};
use crate::scenarios::traffic::create_log;
use crate::scenarios::{Scenario, next_trace_and_span_ids, rng_for_tick};
use rand::prelude::*;

// ============================================================================
// API Reconnaissance Scenario
// ============================================================================

/// Stealthy bot exploring many different API endpoints from a single IP
pub struct APIReconnaissance {
    pub source_ip: String,
    pub attack_rps: f64,
    pub endpoints: Vec<String>,
    trace_id: String,
}

impl APIReconnaissance {
    pub fn new(ip: &str, rps: f64) -> Self {
        let endpoints = vec![
            "/api/v1/user/profile".to_string(),
            "/api/v1/admin/config".to_string(),
            "/api/v1/auth/token".to_string(),
            "/api/v1/debug/dump".to_string(),
            "/api/v1/internal/health".to_string(),
            "/api/v1/payments/history".to_string(),
            "/api/v1/settings/backup".to_string(),
            "/api/v1/db/status".to_string(),
        ];
        
        let hash = xxhash_rust::xxh3::xxh3_64(ip.as_bytes());

        Self {
            source_ip: ip.to_string(),
            attack_rps: rps,
            endpoints,
            trace_id: format!("{:016x}", hash),
        }
    }
}

impl Scenario for APIReconnaissance {
    fn name(&self) -> &str {
        "API Reconnaissance"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rng_for_tick("security/api_recon", current_time_ns, delta_ns);
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.attack_rps * seconds).round() as u64;
        let mut logs = Vec::new();

        for i in 0..count {
            let endpoint = self.endpoints.choose(&mut rng).unwrap();
            let span_id = format!("{:016x}", i);

            // Recon looks like normal 404s or 403s
            let (status, level) = if rng.random_bool(0.8) {
                (404, "WARN")
            } else {
                (403, "ERROR")
            };

            logs.push(create_log(
                level,
                format!("Access denied to {} for {}", endpoint, self.source_ip),
                "api-gateway",
                &self.trace_id,
                &span_id,
                current_time_ns + (i * 1_000_000),
                vec![
                    KeyValue {
                        key: "http.status_code".to_string(),
                        value: AnyValue::int(status),
                    },
                    KeyValue {
                        key: "http.url".to_string(),
                        value: AnyValue::string(endpoint.clone()),
                    },
                    KeyValue {
                        key: "net.peer.ip".to_string(),
                        value: AnyValue::string(self.source_ip.clone()),
                    },
                    KeyValue {
                        key: "threat.category".to_string(),
                        value: AnyValue::string("reconnaissance"),
                    },
                ],
            ));
        }
        logs
    }
}

// --- 1. Credential Stuffing / Brute Force ---
pub struct CredentialStuffing {
    pub attack_rps: f64,
}

impl Scenario for CredentialStuffing {
    fn name(&self) -> &str {
        "Credential Stuffing"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rng_for_tick("security/credential_stuffing", current_time_ns, delta_ns);
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.attack_rps * seconds).round() as u64;
        let mut logs = Vec::new();

        // 80% fail, 20% success (simulating successful breaches mixed in)
        // High cardinality user IDs
        for i in 0..count {
            let (trace_id, span_id) = next_trace_and_span_ids(&mut rng);
            let user_id = format!("user_{}_{}", current_time_ns, i); // Synthetic distinct users
            let is_success = rng.random_bool(0.01); // 1% accidental success in stuffing

            let (level, msg, code) = if is_success {
                ("WARN", "Suspicious login from new location", 200)
            } else {
                ("WARN", "Login failed: Invalid credentials", 401)
            };

            // Actually stuffing usually comes from many IPs.
            // Let's sim a rotating proxy:
            let ip_octet = rng.random_range(1..255);
            let bot_ip = format!(
                "{}.{}.{}.{}",
                rng.random_range(10..200),
                rng.random_range(0..255),
                rng.random_range(0..255),
                ip_octet
            );

            // ANOMALOUS METRICS: Credential stuffing causes:
            // 1. High latency due to auth service overload (300-1000ms vs normal 20-100ms)
            // 2. Burst of requests (high volume)
            // 3. Elevated error rate (401s)
            let attack_latency = rng.random_range(300.0..1000.0); // High latency during attack

            logs.push(create_log(
                level,
                format!("{} for user {}", msg, user_id),
                "auth-service",
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "event.category".to_string(),
                        value: AnyValue::string("authentication"),
                    },
                    KeyValue {
                        key: "user.id".to_string(),
                        value: AnyValue::string(user_id),
                    },
                    KeyValue {
                        key: "source.ip".to_string(),
                        value: AnyValue::string(bot_ip),
                    },
                    KeyValue {
                        key: "http.status_code".to_string(),
                        value: AnyValue::int(code),
                    },
                    // ADDED: Latency metric for statistical detection
                    KeyValue {
                        key: "http.duration_ms".to_string(),
                        value: AnyValue::double(attack_latency),
                    },
                ],
            ));
        }
        logs
    }
}

// --- 2. SQL Injection (SQLi) ---
pub struct SqlInjection {
    pub attack_rps: f64,
}

impl Scenario for SqlInjection {
    fn name(&self) -> &str {
        "SQL Injection Probe"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rng_for_tick("security/sql_injection", current_time_ns, delta_ns);
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.attack_rps * seconds).round() as u64;
        let mut logs = Vec::new();

        let payloads = vec![
            "' OR 1=1 --",
            "UNION SELECT * FROM users",
            "admin' --",
            "1; DROP TABLE users",
        ];

        for _ in 0..count {
            let (trace_id, span_id) = next_trace_and_span_ids(&mut rng);
            let payload = payloads.choose(&mut rng).unwrap();

            // WAF or App log
            logs.push(create_log(
                "ERROR",
                format!("SQL Syntax Error: near \"{}\"", payload),
                "db-cluster",
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "db.statement".to_string(),
                        value: AnyValue::string(format!(
                            "SELECT * FROM products WHERE id = {}",
                            payload
                        )),
                    },
                    KeyValue {
                        key: "security.threat.detected".to_string(),
                        value: AnyValue::bool(true),
                    },
                ],
            ));
        }
        logs
    }
}

// --- 3. Port Scanning ---
pub struct PortScan {
    pub source_ip: String,
    pub scan_speed: f64,
}

impl Scenario for PortScan {
    fn name(&self) -> &str {
        "Port Scan"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rng_for_tick("security/port_scan", current_time_ns, delta_ns);
        let seconds = delta_ns as f64 / 1_000_000_000.0;
        let count = (self.scan_speed * seconds).round() as u64;
        let mut logs = Vec::new();

        let ports = vec![21, 22, 23, 80, 443, 3306, 8080, 5432];

        for _ in 0..count {
            let (trace_id, span_id) = next_trace_and_span_ids(&mut rng);
            let port = ports.choose(&mut rng).unwrap();

            logs.push(create_log(
                "INFO", // Scans often just look like connects
                format!("Connection attempt on port {}", port),
                "firewall-gateway",
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "net.peer.ip".to_string(),
                        value: AnyValue::string(self.source_ip.clone()),
                    },
                    KeyValue {
                        key: "net.host.port".to_string(),
                        value: AnyValue::int(*port),
                    },
                    KeyValue {
                        key: "event.action".to_string(),
                        value: AnyValue::string("allow"),
                    }, // or deny
                ],
            ));
        }
        logs
    }
}
