//! Benchmark Suite for VIA Detection
//!
//! Comprehensive evaluation of all 10 SOTA detectors with proper ground truth tracking:
//! - Precision, Recall, F1-Score per detector
//! - Latency measurements (p50, p95, p99)
//! - Throughput (EPS)
//! - Detection latency (time to detect)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use via_core::engine::AnomalyProfile;
use via_core::signal::{AnomalySignal, DetectorId, NUM_DETECTORS};
use via_sim::{LogRecord, SimulationEngine};

/// Benchmark configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BenchmarkConfig {
    pub name: String,
    pub base_scenario: String,
    pub duration_minutes: u64,
    pub tick_ms: u64,
    pub anomalies: Vec<AnomalySpec>,
    /// Batch size for batch processing mode (0 = single event mode)
    #[serde(default)]
    pub batch_size: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            name: "Default Benchmark".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 5,
            tick_ms: 100,
            anomalies: Vec::new(),
            batch_size: 0, // Single event mode by default
        }
    }
}

/// Anomaly specification for benchmarks
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnomalySpec {
    /// Scenario name (from via-sim scenarios)
    pub scenario: String,
    /// When to start the anomaly (offset from simulation start, in seconds)
    pub start_time_sec: u64,
    /// How long the anomaly lasts (in seconds)
    pub duration_sec: u64,
}

/// Benchmark results with proper metrics
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BenchmarkResults {
    pub config: String,
    pub total_events: u64,
    pub total_anomalies_injected: usize,
    pub total_anomaly_events: u64,
    pub total_detections: u64,

    // Overall accuracy
    pub true_positives: u64,
    pub false_positives: u64,
    pub true_negatives: u64,
    pub false_negatives: u64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,

    // Per-detector breakdown
    pub detector_metrics: HashMap<String, DetectorMetrics>,

    // Performance
    pub latency_micros: LatencyMetrics,
    pub throughput_eps: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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
    pub total_score: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LatencyMetrics {
    pub p50_micros: f64,
    pub p95_micros: f64,
    pub p99_micros: f64,
    pub avg_micros: f64,
}

/// Detection event for tracking
struct DetectionEvent {
    is_ground_truth_anomaly: bool,
    detected_as_anomaly: bool,
    signal: AnomalySignal,
}

/// Main benchmark runner with proper ground truth tracking
pub struct BenchmarkRunner {
    profile: AnomalyProfile,
    detection_events: Vec<DetectionEvent>,
    latencies: Vec<u64>,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            profile: AnomalyProfile::default(),
            detection_events: Vec::new(),
            latencies: Vec::new(),
        }
    }

    pub fn run(&mut self, config: BenchmarkConfig) -> BenchmarkResults {
        let batch_mode = if config.batch_size > 0 {
            format!("Batch Size: {}", config.batch_size)
        } else {
            "Single Event Mode".to_string()
        };

        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║           VIA Benchmark Suite - Ground Truth Mode            ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Config: {:50} ║", config.name);
        println!(
            "║ Duration: {} min | Base: {:35} ║",
            config.duration_minutes, config.base_scenario
        );
        println!(
            "║ Anomalies: {:3} scheduled {:>34} ║",
            config.anomalies.len(),
            ""
        );
        println!("║ Mode: {:52} ║", batch_mode);
        println!("╚══════════════════════════════════════════════════════════════╝");

        let start_time = Instant::now();

        // Create simulation engine
        let mut engine = SimulationEngine::new();
        engine.start(&config.base_scenario);

        // Schedule all anomalies
        for anomaly in &config.anomalies {
            let start_offset_ns = anomaly.start_time_sec * 1_000_000_000;
            let duration_ns = anomaly.duration_sec * 1_000_000_000;
            if let Some(id) =
                engine.schedule_anomaly(&anomaly.scenario, start_offset_ns, duration_ns)
            {
                println!("  Scheduled anomaly '{}' (id: {})", anomaly.scenario, id);
            } else {
                println!("  Warning: Unknown scenario '{}'", anomaly.scenario);
            }
        }

        let duration_ns = config.duration_minutes * 60 * 1_000_000_000;
        let tick_ns = config.tick_ms * 1_000_000;
        let total_ticks = duration_ns / tick_ns;
        let batch_size = config.batch_size;

        println!("\n🔄 Running benchmark... ({} ticks)\n", total_ticks);

        let mut total_events = 0u64;
        let mut _elapsed_ns = 0u64;

        // For batched processing, collect logs first
        let mut pending_logs: Vec<(LogRecord, bool)> = Vec::new();

        for tick in 0..total_ticks {
            let batch = engine.tick(tick_ns);
            _elapsed_ns += tick_ns;

            // Process each log through detection
            for resource_log in &batch.logs.resourceLogs {
                for scope_log in &resource_log.scopeLogs {
                    for log in &scope_log.logRecords {
                        if batch_size > 0 {
                            // Batch mode: collect logs
                            pending_logs.push((log.clone(), log.isGroundTruthAnomaly));

                            // Process batch when full
                            if pending_logs.len() >= batch_size {
                                self.process_batch(&pending_logs);
                                pending_logs.clear();
                            }
                        } else {
                            // Single event mode
                            self.process_log(log);
                        }
                    }
                    total_events += scope_log.logRecords.len() as u64;
                }
            }

            // Progress update every 10% or 100 ticks
            if tick % (total_ticks / 10).max(100) == 0 {
                let progress = ((tick + 1) as f64 / total_ticks as f64 * 100.0) as u32;
                print!(
                    "\r  [{:>3}%] Tick {:>6}/{} | {:>8} events",
                    progress,
                    tick + 1,
                    total_ticks,
                    total_events
                );
            }
        }

        // Process remaining logs in batch mode
        if !pending_logs.is_empty() {
            self.process_batch(&pending_logs);
        }

        println!(
            "\n\n✅ Benchmark completed in {:.2}s",
            start_time.elapsed().as_secs_f64()
        );

        // Calculate results
        self.calculate_results(&config, total_events, start_time.elapsed())
    }

    /// Process a batch of logs (amortizes overhead)
    fn process_batch(&mut self, logs: &[(LogRecord, bool)]) {
        let start = Instant::now();

        for (log, is_anomaly) in logs {
            let value = log.metric_value();
            let timestamp: u64 = log.timeUnixNano.parse().unwrap_or(0);
            let entity_hash = xxhash_rust::xxh3::xxh3_64(log.traceId.as_bytes());

            let signal = self
                .profile
                .process_with_hash(timestamp, entity_hash, value);

            self.detection_events.push(DetectionEvent {
                is_ground_truth_anomaly: *is_anomaly,
                detected_as_anomaly: signal.is_anomaly,
                signal,
            });
        }

        // Record batch latency (divided by batch size for per-event latency)
        let elapsed_per_event = start.elapsed().as_micros() as u64 / logs.len().max(1) as u64;
        self.latencies.push(elapsed_per_event);
    }

    fn process_log(&mut self, log: &LogRecord) {
        let start = Instant::now();

        // Extract value for detection
        let value = log.metric_value();
        let timestamp: u64 = log.timeUnixNano.parse().unwrap_or(0);
        let entity_hash = xxhash_rust::xxh3::xxh3_64(log.traceId.as_bytes());

        // Run detection - get full AnomalySignal
        let signal = self
            .profile
            .process_with_hash(timestamp, entity_hash, value);

        let elapsed = start.elapsed();
        self.latencies.push(elapsed.as_micros() as u64);

        // Store detection event - ground truth comes from the log itself
        self.detection_events.push(DetectionEvent {
            is_ground_truth_anomaly: log.isGroundTruthAnomaly,
            detected_as_anomaly: signal.is_anomaly,
            signal,
        });
    }

    fn calculate_results(
        &self,
        config: &BenchmarkConfig,
        total_events: u64,
        elapsed: std::time::Duration,
    ) -> BenchmarkResults {
        // Calculate overall TP/FP/TN/FN
        let mut tp = 0u64;
        let mut fp = 0u64;
        let mut tn = 0u64;
        let mut fn_ = 0u64;
        let mut anomaly_events = 0u64;

        for event in &self.detection_events {
            if event.is_ground_truth_anomaly {
                anomaly_events += 1;
            }

            match (event.detected_as_anomaly, event.is_ground_truth_anomaly) {
                (true, true) => tp += 1,
                (true, false) => fp += 1,
                (false, true) => fn_ += 1,
                (false, false) => tn += 1,
            }
        }

        let (precision, recall, f1) = calculate_metrics(tp, fp, fn_);

        // Calculate per-detector metrics
        let mut detector_metrics = HashMap::new();

        for detector_id in 0..NUM_DETECTORS {
            if let Some(id) = DetectorId::from_u8(detector_id as u8) {
                let name = id.name().to_string();
                let mut dm = DetectorMetrics {
                    name: name.clone(),
                    ..Default::default()
                };

                // Calculate per-detector TP/FP/TN/FN based on which detector fired
                for event in &self.detection_events {
                    let detector_fired = event.signal.detector_scores[detector_id].fired;

                    match (detector_fired, event.is_ground_truth_anomaly) {
                        (true, true) => dm.true_positives += 1,
                        (true, false) => dm.false_positives += 1,
                        (false, true) => dm.false_negatives += 1,
                        (false, false) => dm.true_negatives += 1,
                    }

                    if detector_fired {
                        dm.trigger_count += 1;
                    }
                    dm.total_score += event.signal.detector_scores[detector_id].score as f64;
                }

                let (p, r, f) =
                    calculate_metrics(dm.true_positives, dm.false_positives, dm.false_negatives);
                dm.precision = p;
                dm.recall = r;
                dm.f1_score = f;
                dm.avg_score = if self.detection_events.is_empty() {
                    0.0
                } else {
                    dm.total_score / self.detection_events.len() as f64
                };

                detector_metrics.insert(name, dm);
            }
        }

        // Calculate latency metrics
        let latency_micros = self.calculate_latency_metrics();

        BenchmarkResults {
            config: config.name.clone(),
            total_events,
            total_anomalies_injected: config.anomalies.len(),
            total_anomaly_events: anomaly_events,
            total_detections: tp + fp,
            true_positives: tp,
            false_positives: fp,
            true_negatives: tn,
            false_negatives: fn_,
            precision,
            recall,
            f1_score: f1,
            detector_metrics,
            latency_micros,
            throughput_eps: total_events as f64 / elapsed.as_secs_f64(),
        }
    }

    fn calculate_latency_metrics(&self) -> LatencyMetrics {
        if self.latencies.is_empty() {
            return LatencyMetrics::default();
        }

        let mut sorted = self.latencies.clone();
        sorted.sort();

        let len = sorted.len();
        let avg = sorted.iter().sum::<u64>() as f64 / len as f64;
        let p50 = sorted[len / 2] as f64;
        let p95 = sorted[len * 95 / 100] as f64;
        let p99 = sorted[len * 99 / 100] as f64;

        LatencyMetrics {
            p50_micros: p50,
            p95_micros: p95,
            p99_micros: p99,
            avg_micros: avg,
        }
    }

    pub fn print_results(&self, results: &BenchmarkResults) {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                    BENCHMARK RESULTS                         ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Configuration: {:44} ║", results.config);
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ OVERALL METRICS                                              ║");
        println!("╠──────────────────────────────────────────────────────────────╣");
        println!(
            "║ Total Events:       {:>10}                              ║",
            results.total_events
        );
        println!(
            "║ Anomaly Events:     {:>10}                              ║",
            results.total_anomaly_events
        );
        println!(
            "║ Detections:         {:>10}                              ║",
            results.total_detections
        );
        println!(
            "║ Throughput:         {:>10.0} EPS                          ║",
            results.throughput_eps
        );
        println!("╠──────────────────────────────────────────────────────────────╣");
        println!("║ ACCURACY                                                     ║");
        println!("╠──────────────────────────────────────────────────────────────╣");
        println!(
            "║ True Positives:     {:>10}                              ║",
            results.true_positives
        );
        println!(
            "║ False Positives:    {:>10}                              ║",
            results.false_positives
        );
        println!(
            "║ True Negatives:     {:>10}                              ║",
            results.true_negatives
        );
        println!(
            "║ False Negatives:    {:>10}                              ║",
            results.false_negatives
        );
        println!("║                                                              ║");
        println!(
            "║ Precision:          {:>10.2}%                             ║",
            results.precision * 100.0
        );
        println!(
            "║ Recall:             {:>10.2}%                             ║",
            results.recall * 100.0
        );
        println!(
            "║ F1-Score:           {:>10.3}                              ║",
            results.f1_score
        );
        println!("╠──────────────────────────────────────────────────────────────╣");
        println!("║ LATENCY (microseconds)                                       ║");
        println!("╠──────────────────────────────────────────────────────────────╣");
        println!(
            "║ Average:            {:>10.2} µs                           ║",
            results.latency_micros.avg_micros
        );
        println!(
            "║ P50:                {:>10.2} µs                           ║",
            results.latency_micros.p50_micros
        );
        println!(
            "║ P95:                {:>10.2} µs                           ║",
            results.latency_micros.p95_micros
        );
        println!(
            "║ P99:                {:>10.2} µs                           ║",
            results.latency_micros.p99_micros
        );
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ PER-DETECTOR BREAKDOWN                                       ║");
        println!("╠──────────────────────────────────────────────────────────────╣");

        for (name, metrics) in &results.detector_metrics {
            if metrics.trigger_count > 0 {
                println!(
                    "║ {:24} | P: {:5.1}% | R: {:5.1}% | F1: {:5.3} ║",
                    name,
                    metrics.precision * 100.0,
                    metrics.recall * 100.0,
                    metrics.f1_score
                );
            }
        }

        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    pub fn export_json(&self, results: &BenchmarkResults) -> String {
        serde_json::to_string_pretty(results).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate precision, recall, f1 from confusion matrix values
pub fn calculate_metrics(tp: u64, fp: u64, fn_: u64) -> (f64, f64, f64) {
    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        0.0
    };
    let recall = if tp + fn_ > 0 {
        tp as f64 / (tp + fn_) as f64
    } else {
        0.0
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    (precision, recall, f1)
}

/// Predefined benchmark scenarios
pub mod scenarios {
    use super::*;

    /// Standard mixed workload with various anomalies
    pub fn mixed_workload() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Mixed Workload - All Anomaly Types".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 5,
            tick_ms: 100,
            anomalies: vec![
                AnomalySpec {
                    scenario: "credential_stuffing".to_string(),
                    start_time_sec: 60,
                    duration_sec: 60,
                },
                AnomalySpec {
                    scenario: "memory_leak".to_string(),
                    start_time_sec: 180,
                    duration_sec: 120,
                },
                AnomalySpec {
                    scenario: "traffic_spike".to_string(),
                    start_time_sec: 240,
                    duration_sec: 30,
                },
            ],
            ..Default::default()
        }
    }

    /// Security-focused benchmark
    pub fn security_audit() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Security Audit - Attack Detection".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 3,
            tick_ms: 50,
            anomalies: vec![
                AnomalySpec {
                    scenario: "credential_stuffing".to_string(),
                    start_time_sec: 30,
                    duration_sec: 60,
                },
                AnomalySpec {
                    scenario: "sql_injection".to_string(),
                    start_time_sec: 120,
                    duration_sec: 45,
                },
            ],
            ..Default::default()
        }
    }

    /// Performance-focused benchmark
    pub fn performance_stress() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Performance Stress Test".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 5,
            tick_ms: 50,
            anomalies: vec![
                AnomalySpec {
                    scenario: "cpu_spike".to_string(),
                    start_time_sec: 60,
                    duration_sec: 180,
                },
                AnomalySpec {
                    scenario: "slow_queries".to_string(),
                    start_time_sec: 180,
                    duration_sec: 120,
                },
            ],
            ..Default::default()
        }
    }

    /// High throughput benchmark (no anomalies)
    pub fn throughput_test() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Throughput Test".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 2,
            tick_ms: 10,
            anomalies: vec![],
            ..Default::default()
        }
    }

    /// Cascade failure scenario
    pub fn cascade_failure() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Cascade Failure Detection".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 4,
            tick_ms: 100,
            anomalies: vec![AnomalySpec {
                scenario: "cascade_failure".to_string(),
                start_time_sec: 90,
                duration_sec: 60,
            }],
            ..Default::default()
        }
    }

    /// Quick validation benchmark
    pub fn quick_validation() -> BenchmarkConfig {
        BenchmarkConfig {
            name: "Quick Validation".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 1,
            tick_ms: 100,
            anomalies: vec![AnomalySpec {
                scenario: "traffic_spike".to_string(),
                start_time_sec: 15,
                duration_sec: 15,
            }],
            ..Default::default()
        }
    }
}
