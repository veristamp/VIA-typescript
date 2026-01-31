//! Benchmark Suite for VIA Detection
//!
//! Comprehensive evaluation of all 10 SOTA detectors:
//! - Precision, Recall, F1-Score per detector
//! - Latency measurements (p50, p95, p99)
//! - Throughput (EPS)
//! - False positive rate
//! - Detection latency (time to detect)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use via_core::engine::AnomalyProfile;
use via_sim::generator::{topologies, AnomalyConfig, AnomalyType, LogGenerator, OTelLogRecord};

/// Benchmark configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BenchmarkConfig {
    pub name: String,
    pub topology: Topology,
    pub duration_minutes: u64,
    pub window_size_sec: u64,
    pub anomalies: Vec<AnomalyConfig>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Topology {
    Microservices,
    DataPipeline,
    Infrastructure,
}

/// Benchmark results
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BenchmarkResults {
    pub config: String,
    pub total_events: u64,
    pub total_anomalies_injected: u64,
    pub total_anomalies_detected: u64,
    pub detector_metrics: HashMap<String, DetectorMetrics>,
    pub latency_micros: LatencyMetrics,
    pub throughput_eps: f64,
    pub false_positive_rate: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DetectorMetrics {
    pub name: String,
    pub true_positives: u64,
    pub false_positives: u64,
    pub true_negatives: u64,
    pub false_negatives: u64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub avg_score: f64,
    pub trigger_count: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LatencyMetrics {
    pub p50_micros: f64,
    pub p95_micros: f64,
    pub p99_micros: f64,
    pub avg_micros: f64,
}

/// Main benchmark runner
pub struct BenchmarkRunner {
    profile: AnomalyProfile,
    results: BenchmarkResults,
    latencies: Vec<u64>,            // Microseconds
    ground_truth: Vec<(u64, bool)>, // (timestamp, is_anomaly)
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            profile: AnomalyProfile::default(),
            results: BenchmarkResults {
                config: "default".to_string(),
                total_events: 0,
                total_anomalies_injected: 0,
                total_anomalies_detected: 0,
                detector_metrics: HashMap::new(),
                latency_micros: LatencyMetrics {
                    p50_micros: 0.0,
                    p95_micros: 0.0,
                    p99_micros: 0.0,
                    avg_micros: 0.0,
                },
                throughput_eps: 0.0,
                false_positive_rate: 0.0,
            },
            latencies: Vec::new(),
            ground_truth: Vec::new(),
        }
    }

    pub fn run(&mut self, config: BenchmarkConfig) -> BenchmarkResults {
        println!("Starting benchmark: {}", config.name);
        println!("Duration: {} minutes", config.duration_minutes);
        println!("Window size: {} seconds", config.window_size_sec);

        let start_time = Instant::now();
        let mut generator = match config.topology {
            Topology::Microservices => LogGenerator::new(topologies::microservices()),
            Topology::DataPipeline => LogGenerator::new(topologies::data_pipeline()),
            Topology::Infrastructure => LogGenerator::new(topologies::infrastructure()),
        };

        // Inject anomalies
        for anomaly in &config.anomalies {
            generator.inject_anomaly(anomaly.clone());
            self.results.total_anomalies_injected += 1;
        }

        let windows = (config.duration_minutes * 60) / config.window_size_sec;

        for window in 0..windows {
            let window_start = Instant::now();

            // Generate logs for this window
            let logs = generator.generate_window(config.window_size_sec);

            // Process each log through detection
            for log in &logs {
                self.process_log(log);
            }

            let window_elapsed = window_start.elapsed();
            let eps = logs.len() as f64 / window_elapsed.as_secs_f64();

            if window % 10 == 0 {
                println!(
                    "Window {}/{}: {} events, {:.0} EPS",
                    window,
                    windows,
                    logs.len(),
                    eps
                );
            }
        }

        let total_elapsed = start_time.elapsed();
        self.results.total_events = generator.get_stats().total_events;
        self.results.throughput_eps =
            self.results.total_events as f64 / total_elapsed.as_secs_f64();

        // Calculate latency percentiles
        self.calculate_latency_metrics();

        // Calculate detector metrics
        self.calculate_detector_metrics();

        println!("\nBenchmark completed!");
        println!("Total events: {}", self.results.total_events);
        println!("Throughput: {:.0} EPS", self.results.throughput_eps);

        self.results.clone()
    }

    fn process_log(&mut self, log: &OTelLogRecord) {
        let start = Instant::now();

        // Extract value for detection (latency, memory, etc.)
        let value = self.extract_metric_value(log);
        let timestamp = log.timestamp.timestamp_nanos_opt().unwrap_or(0) as u64;
        let entity_id = log.trace_id.clone();

        // Run detection
        let result = self.profile.process(timestamp, &entity_id, value);

        let elapsed = start.elapsed();
        self.latencies.push(elapsed.as_micros() as u64);

        // Check if this was an actual anomaly (ground truth)
        let is_actual_anomaly = self.is_log_anomalous(log);

        // Track detection performance
        if result.is_anomaly {
            self.results.total_anomalies_detected += 1;
        }

        // Store ground truth for metrics calculation
        self.ground_truth.push((timestamp, is_actual_anomaly));
    }

    fn extract_metric_value(&self, log: &OTelLogRecord) -> f64 {
        // Try to find a numeric metric in attributes
        for (key, value) in &log.attributes {
            match key.as_str() {
                "http.duration_ms" => {
                    if let via_sim::generator::AttributeValue::Double(v) = value {
                        return *v;
                    }
                    if let via_sim::generator::AttributeValue::Int(v) = value {
                        return *v as f64;
                    }
                }
                "process.memory.usage" => {
                    if let via_sim::generator::AttributeValue::Double(v) = value {
                        return *v;
                    }
                }
                "http.status_code" => {
                    if let via_sim::generator::AttributeValue::Int(v) = value {
                        return *v as f64;
                    }
                }
                _ => {}
            }
        }

        // Default: use body length as proxy
        log.body.len() as f64
    }

    fn is_log_anomalous(&self, _log: &OTelLogRecord) -> bool {
        // This would check against the anomaly configs to see if this log
        // was part of an injected anomaly
        // For now, simplified
        false
    }

    fn calculate_latency_metrics(&mut self) {
        if self.latencies.is_empty() {
            return;
        }

        let mut sorted = self.latencies.clone();
        sorted.sort();

        let len = sorted.len();
        let avg = sorted.iter().sum::<u64>() as f64 / len as f64;
        let p50 = sorted[len / 2] as f64;
        let p95 = sorted[len * 95 / 100] as f64;
        let p99 = sorted[len * 99 / 100] as f64;

        self.results.latency_micros = LatencyMetrics {
            p50_micros: p50,
            p95_micros: p95,
            p99_micros: p99,
            avg_micros: avg,
        };
    }

    fn calculate_detector_metrics(&mut self) {
        // This would analyze the ground truth vs detections per detector
        // For now, simplified placeholder
        let detectors = vec![
            "Volume/RPS",
            "Distribution/Latency",
            "Cardinality/Velocity",
            "Burst/IAT",
            "Spectral/FFT",
            "ChangePoint/Trend",
            "RRCF/Multivariate",
            "MultiScale/Temporal",
            "Behavioral/Fingerprint",
            "Drift/Concept",
        ];

        for name in detectors {
            self.results.detector_metrics.insert(
                name.to_string(),
                DetectorMetrics {
                    name: name.to_string(),
                    true_positives: 0,
                    false_positives: 0,
                    true_negatives: 0,
                    false_negatives: 0,
                    precision: 0.0,
                    recall: 0.0,
                    f1_score: 0.0,
                    avg_score: 0.0,
                    trigger_count: 0,
                },
            );
        }
    }

    pub fn print_results(&self) {
        println!("\n========== BENCHMARK RESULTS ==========\n");
        println!("Configuration: {}", self.results.config);
        println!("Total Events: {}", self.results.total_events);
        println!("Throughput: {:.0} EPS", self.results.throughput_eps);
        println!("\nLatency (microseconds):");
        println!("  Average: {:.2}", self.results.latency_micros.avg_micros);
        println!("  P50: {:.2}", self.results.latency_micros.p50_micros);
        println!("  P95: {:.2}", self.results.latency_micros.p95_micros);
        println!("  P99: {:.2}", self.results.latency_micros.p99_micros);

        println!("\nDetector Performance:");
        for (name, metrics) in &self.results.detector_metrics {
            println!("  {}:", name);
            println!("    Precision: {:.2}%", metrics.precision * 100.0);
            println!("    Recall: {:.2}%", metrics.recall * 100.0);
            println!("    F1-Score: {:.2}", metrics.f1_score);
        }
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.results).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Predefined benchmark scenarios
pub mod scenarios {
    use super::*;
    use via_sim::generator::AnomalyConfig;

    /// Standard mixed workload with various anomalies
    pub fn mixed_workload() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Mixed Workload - All Anomaly Types".to_string(),
            topology: Topology::Microservices,
            duration_minutes: 5,
            window_size_sec: 10,
            anomalies: vec![
                AnomalyConfig {
                    anomaly_type: AnomalyType::CredentialStuffing {
                        attempts_per_sec: 100.0,
                    },
                    start_time_sec: 60,
                    duration_sec: 60,
                    severity: 0.8,
                    target_services: vec!["auth-service".to_string()],
                },
                AnomalyConfig {
                    anomaly_type: AnomalyType::MemoryLeak {
                        leak_rate_mb_per_sec: 50.0,
                    },
                    start_time_sec: 180,
                    duration_sec: 120,
                    severity: 0.9,
                    target_services: vec!["payment-service".to_string()],
                },
                AnomalyConfig {
                    anomaly_type: AnomalyType::TrafficSpike { multiplier: 10.0 },
                    start_time_sec: 240,
                    duration_sec: 30,
                    severity: 0.7,
                    target_services: vec!["api-gateway".to_string()],
                },
            ],
        }
    }

    /// Security-focused benchmark
    pub fn security_audit() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Security Audit - All Attack Types".to_string(),
            topology: Topology::Microservices,
            duration_minutes: 3,
            window_size_sec: 5,
            anomalies: vec![
                AnomalyConfig {
                    anomaly_type: AnomalyType::CredentialStuffing {
                        attempts_per_sec: 500.0,
                    },
                    start_time_sec: 30,
                    duration_sec: 60,
                    severity: 0.9,
                    target_services: vec!["auth-service".to_string()],
                },
                AnomalyConfig {
                    anomaly_type: AnomalyType::SqlInjection { probe_rate: 50.0 },
                    start_time_sec: 120,
                    duration_sec: 45,
                    severity: 0.85,
                    target_services: vec!["inventory-service".to_string()],
                },
            ],
        }
    }

    /// Performance-focused benchmark
    pub fn performance_stress() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Performance Stress Test".to_string(),
            topology: Topology::DataPipeline,
            duration_minutes: 5,
            window_size_sec: 5,
            anomalies: vec![
                AnomalyConfig {
                    anomaly_type: AnomalyType::CpuSaturation { intensity: 0.9 },
                    start_time_sec: 60,
                    duration_sec: 180,
                    severity: 0.8,
                    target_services: vec!["stream-processor".to_string()],
                },
                AnomalyConfig {
                    anomaly_type: AnomalyType::SlowQueries {
                        latency_multiplier: 10.0,
                    },
                    start_time_sec: 180,
                    duration_sec: 120,
                    severity: 0.75,
                    target_services: vec!["analytics-api".to_string()],
                },
            ],
        }
    }

    /// High throughput benchmark
    pub fn throughput_test() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Maximum Throughput Test".to_string(),
            topology: Topology::Infrastructure,
            duration_minutes: 2,
            window_size_sec: 1,
            anomalies: vec![], // No anomalies, pure throughput
        }
    }
}
