use crate::simulation::scenarios::traffic::create_log;
use crate::simulation::scenarios::Scenario;
use crate::simulation::types::{AnyValue, KeyValue, LogRecord};
use rand::prelude::*;
use rand_distr::{Distribution, Normal};
use uuid::Uuid;

// --- 1. Memory Leak ---
pub struct MemoryLeak {
    pub service_name: String,
    pub leak_rate_mb_per_sec: f64,
    pub max_memory_mb: f64,
    current_memory_mb: f64,
    has_crashed: bool,
}

impl MemoryLeak {
    pub fn new(service_name: &str, leak_rate: f64) -> Self {
        Self {
            service_name: service_name.to_string(),
            leak_rate_mb_per_sec: leak_rate,
            max_memory_mb: 4096.0,    // 4GB Limit
            current_memory_mb: 256.0, // Start low
            has_crashed: false,
        }
    }
}

impl Scenario for MemoryLeak {
    fn name(&self) -> &str {
        "Memory Leak"
    }

    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord> {
        if self.has_crashed {
            // Restart sequence
            self.current_memory_mb = 256.0;
            self.has_crashed = false;
            return vec![create_log(
                "INFO",
                format!("Service {} restarted successfully.", self.service_name),
                &self.service_name,
                &Uuid::new_v4().simple().to_string(),
                &Uuid::new_v4().simple().to_string()[..16],
                current_time_ns,
                vec![KeyValue {
                    key: "event".to_string(),
                    value: AnyValue::string("service_start"),
                }],
            )];
        }

        let seconds = delta_ns as f64 / 1_000_000_000.0;
        self.current_memory_mb += self.leak_rate_mb_per_sec * seconds;

        let mut rng = rand::rng();
        let mut logs = Vec::new();

        // Generate metric-like logs every second (probabilistically)
        if rng.random_bool(0.2) {
            // not every tick, but frequent
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            let level = if self.current_memory_mb > self.max_memory_mb * 0.9 {
                "FATAL"
            } else if self.current_memory_mb > self.max_memory_mb * 0.75 {
                "WARN"
            } else {
                "INFO"
            };

            let body = format!(
                "Memory usage: {:.2} MB / {:.2} MB",
                self.current_memory_mb, self.max_memory_mb
            );

            logs.push(create_log(
                level,
                body,
                &self.service_name,
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "process.memory.usage".to_string(),
                        value: AnyValue::double(self.current_memory_mb),
                    },
                    KeyValue {
                        key: "process.memory.limit".to_string(),
                        value: AnyValue::double(self.max_memory_mb),
                    },
                ],
            ));
        }

        if self.current_memory_mb >= self.max_memory_mb {
            self.has_crashed = true;
            logs.push(create_log(
                "FATAL",
                "OutOfMemoryError: Java heap space".to_string(),
                &self.service_name,
                &Uuid::new_v4().simple().to_string(),
                &Uuid::new_v4().simple().to_string()[..16],
                current_time_ns,
                vec![KeyValue {
                    key: "error.type".to_string(),
                    value: AnyValue::string("OOM"),
                }],
            ));
        }

        logs
    }
}

// --- 2. CPU Spike ---
pub struct CpuSpike {
    pub service_name: String,
    pub intensity: f64, // 0.0 to 1.0
}

impl CpuSpike {
    pub fn new(service_name: &str, intensity: f64) -> Self {
        Self {
            service_name: service_name.to_string(),
            intensity,
        }
    }
}

impl Scenario for CpuSpike {
    fn name(&self) -> &str {
        "CPU Spike"
    }

    fn tick(&mut self, current_time_ns: u64, _delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        let mut logs = Vec::new();

        // If intensity is high, we generate logs indicating slow processing or thread locking
        if rng.random_bool(self.intensity) {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            // High CPU usually manifests as timeouts or slow processing
            let duration = Normal::new(5000.0, 1000.0).unwrap().sample(&mut rng) as i64;

            logs.push(create_log(
                "WARN",
                format!(
                    "Thread pool exhaustion: Active threads > 95% (Processing time: {}ms)",
                    duration
                ),
                &self.service_name,
                &trace_id,
                &span_id,
                current_time_ns,
                vec![
                    KeyValue {
                        key: "process.cpu.utilization".to_string(),
                        value: AnyValue::double(99.9),
                    },
                    KeyValue {
                        key: "thread.active_count".to_string(),
                        value: AnyValue::int(200),
                    },
                ],
            ));
        }
        logs
    }
}

// --- 3. Infinite Loop (Stack Overflow Simulation) ---
pub struct InfiniteLoop {
    pub service_name: String,
}

impl Scenario for InfiniteLoop {
    fn name(&self) -> &str {
        "Infinite Loop"
    }

    fn tick(&mut self, current_time_ns: u64, _delta_ns: u64) -> Vec<LogRecord> {
        let mut rng = rand::rng();
        // Rare but catastrophic event
        if rng.random_bool(0.05) {
            let trace_id = Uuid::new_v4().simple().to_string();
            let span_id = Uuid::new_v4().simple().to_string()[..16].to_string();

            vec![create_log(
                "ERROR",
                "StackOverflowError: at com.via.algo.Recursive.call(Recursive.java:42)".to_string(),
                &self.service_name,
                &trace_id,
                &span_id,
                current_time_ns,
                vec![KeyValue {
                    key: "error.stack_depth".to_string(),
                    value: AnyValue::int(1024),
                }],
            )]
        } else {
            vec![]
        }
    }
}
