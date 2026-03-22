use crate::core::{AnyValue, KeyValue, LogRecord};
use crate::scenarios::{rng_for_init, Scenario, next_trace_and_span_ids, rng_for_tick};
use rand::prelude::*;
use rand_distr::{Distribution, LogNormal, Normal};

// Shared helper for creating logs to reduce duplication
pub fn create_log(
    level: &str,
    body: String,
    service_name: &str,
    trace_id: &str,
    span_id: &str,
    time_ns: u64,
    mut attributes: Vec<KeyValue>,
) -> LogRecord {
    let severity_number = match level {
        "DEBUG" => 5,
        "INFO" => 9,
        "WARN" => 13,
        "ERROR" => 17,
        "FATAL" => 21,
        _ => 9,
    };

    attributes.push(KeyValue {
        key: "service.name".to_string(),
        value: AnyValue::string(service_name),
    });

    LogRecord {
        timeUnixNano: time_ns.to_string(),
        traceId: trace_id.to_string(),
        spanId: span_id.to_string(),
        severityNumber: severity_number,
        severityText: level.to_string(),
        body: AnyValue::string(body),
        attributes,
        isGroundTruthAnomaly: false,
        anomalyId: None,
    }
}

pub struct NormalTraffic {
    pub logs_per_sec: f64,
    pub services: Vec<String>,
    /// Pool of persistent user identities for behavioral consistency
    user_pool: Vec<String>,
}

impl NormalTraffic {
    pub fn new(logs_per_sec: f64) -> Self {
        let mut rng = rng_for_init("traffic/normal");
        let user_count = 20; // Faster maturity for benchmarks
        let user_pool: Vec<String> = (0..user_count)
            .map(|_| {
                let (trace_id, _) = next_trace_and_span_ids(&mut rng);
                trace_id
            })
            .collect();

        Self {
            logs_per_sec,
            services: vec![
                "auth-service".to_string(),
                "payment-service".to_string(),
                "api-gateway".to_string(),
                "db-cluster".to_string(),
                "inventory-service".to_string(),
                "recommendation-engine".to_string(),
            ],
            user_pool,
        }
    }
}

impl Scenario for NormalTraffic {
    fn name(&self) -> &str {
        "Normal Traffic"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rng_for_tick("traffic/normal", current_time_ns, delta_ns);
        let seconds = delta_ns as f64 / 1_000_000_000.0;

        // Add some jitter to the volume (Poisson-like)
        let vol_dist = Normal::new(self.logs_per_sec, self.logs_per_sec * 0.1).unwrap();
        let count = (vol_dist.sample(&mut rng) * seconds).max(0.0).round() as u64;

        let mut logs = Vec::new();

        for i in 0..count {
            let service = self.services.choose(&mut rng).unwrap();
            
            // SOTA: Identity Persistence
            let user_idx = (i as usize + (current_time_ns / 1_000_000) as usize) % self.user_pool.len();
            let trace_id = &self.user_pool[user_idx];
            let span_id = format!("{:016x}", i);

            // LogNormal for realistic latency tail
            let latency_dist = LogNormal::new(4.0, 0.5).unwrap(); // ~55ms mean
            let latency = latency_dist.sample(&mut rng) as i64;

            // Realistic response size distribution
            let size_dist = LogNormal::new(6.0, 0.2).unwrap(); // mean ~400 bytes
            let response_size = size_dist.sample(&mut rng) as i64;

            let status_code = if rng.random_bool(0.99) { 200 } else { 500 };
            let level = if status_code == 200 { "INFO" } else { "ERROR" };

            let mut attrs = vec![
                KeyValue {
                    key: "http.method".to_string(),
                    value: AnyValue::string("GET"),
                },
                KeyValue {
                    key: "http.status_code".to_string(),
                    value: AnyValue::int(status_code),
                },
                KeyValue {
                    key: "http.duration_ms".to_string(),
                    value: AnyValue::int(latency),
                },
                KeyValue {
                    key: "http.response.body.size".to_string(),
                    value: AnyValue::int(response_size),
                },
                KeyValue {
                    key: "net.peer.ip".to_string(),
                    value: AnyValue::string(format!(
                        "10.0.{}.{}",
                        rng.random_range(0..255),
                        rng.random_range(0..255)
                    )),
                },
            ];

            if status_code == 500 {
                attrs.push(KeyValue {
                    key: "error.type".to_string(),
                    value: AnyValue::string("InternalServerError"),
                });
            }

            let body = format!("Request processed in {}ms", latency);
            let _log_ts = current_time_ns + (i * delta_ns / count.max(1));

            logs.push(create_log(
                level,
                body,
                service,
                trace_id,
                &span_id,
                _log_ts,
                attrs,
            ));
        }
        logs
    }
}
