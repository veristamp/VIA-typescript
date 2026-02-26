use crate::{AnomalySpec, BenchmarkConfig, calculate_metrics, scenarios};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use via_core::engine::AnomalyProfile;
use via_sim::{LogRecord, SimulationEngine};

#[derive(Clone, Debug)]
pub struct PipelineBenchmarkConfig {
    pub benchmark: BenchmarkConfig,
    pub tier2_base_url: String,
    pub send_batch_size: usize,
    pub drain_timeout_secs: u64,
}

impl Default for PipelineBenchmarkConfig {
    fn default() -> Self {
        Self {
            benchmark: scenarios::quick_validation(),
            tier2_base_url: "http://127.0.0.1:3000".to_string(),
            send_batch_size: 256,
            drain_timeout_secs: 900,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PipelineBenchmarkResults {
    pub run_id: String,
    pub config_name: String,
    pub total_events: u64,
    pub total_ground_truth_anomaly_events: u64,
    pub total_detected_anomalies: u64,
    pub tier2_events_sent: u64,

    pub detection_precision: f64,
    pub detection_recall: f64,
    pub detection_f1: f64,
    pub detection_latency_p50_micros: f64,
    pub detection_latency_p95_micros: f64,

    pub incident_precision: f64,
    pub incident_recall: f64,
    pub incident_f1: f64,
    pub merge_error_rate: f64,
    pub split_error_rate: f64,
    pub escalation_quality: f64,

    pub throughput_eps: f64,
    pub cost_per_10k_events_seconds: f64,
    pub anomaly_breakdown: Vec<AnomalyDetectionBreakdown>,
}

#[derive(Serialize, Clone, Debug)]
struct Tier2Signal {
    event_id: String,
    schema_version: u16,
    entity_hash: String,
    timestamp: String,
    score: f64,
    severity: u8,
    primary_detector: u8,
    detectors_fired: u8,
    confidence: f64,
    detector_scores: Vec<f64>,
    attributes: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnomalyDetectionBreakdown {
    pub anomaly_id: String,
    pub scenario: String,
    pub scheduled_start_sec: u64,
    pub scheduled_duration_sec: u64,
    pub ground_truth_events: u64,
    pub detected_events: u64,
    pub missed_events: u64,
    pub recall: f64,
}

#[derive(Clone, Debug)]
struct ScheduledAnomalyManifest {
    anomaly_id: String,
    scenario: String,
    start_time_sec: u64,
    duration_sec: u64,
}

#[derive(Default, Clone, Debug)]
struct AnomalyEventStats {
    ground_truth_events: u64,
    detected_events: u64,
}

pub struct PipelineBenchmarkRunner {
    profile: AnomalyProfile,
    client: Client,
}

impl PipelineBenchmarkRunner {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("failed to create HTTP client: {e}"))?;

        Ok(Self {
            profile: AnomalyProfile::default(),
            client,
        })
    }

    fn send_batch(&self, base_url: &str, signals: &[Tier2Signal]) -> Result<(), String> {
        if signals.is_empty() {
            return Ok(());
        }

        let url = format!("{}/tier2/anomalies", base_url.trim_end_matches('/'));
        let body = json!({ "signals": signals });

        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .map_err(|e| format!("tier2 ingest request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .unwrap_or_else(|_| "<body-unavailable>".to_string());
            return Err(format!("tier2 ingest failed with status {status}: {text}"));
        }

        Ok(())
    }

    fn wait_for_pipeline_drain(&self, base_url: &str, timeout_secs: u64) -> Result<(), String> {
        let url = format!("{}/analysis/pipeline/stats", base_url.trim_end_matches('/'));
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        while Instant::now() < deadline {
            let response = self
                .client
                .get(&url)
                .send()
                .map_err(|e| format!("pipeline stats request failed: {e}"))?;

            if !response.status().is_success() {
                return Err(format!(
                    "pipeline stats request failed: status {}",
                    response.status()
                ));
            }

            let value: Value = response
                .json()
                .map_err(|e| format!("invalid pipeline stats JSON: {e}"))?;

            let queued = value["queue"]["queued"].as_u64().unwrap_or(0);
            let in_flight = value["queue"]["inFlight"].as_u64().unwrap_or(0);

            if queued == 0 && in_flight == 0 {
                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        Err("timeout waiting for tier2 pipeline to drain".to_string())
    }

    fn fetch_incidents(&self, base_url: &str, run_id: &str) -> Result<Vec<Value>, String> {
        let url = format!(
            "{}/analysis/incidents/run/{}?limit=100000",
            base_url.trim_end_matches('/'),
            run_id
        );

        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| format!("incident list request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "incident list request failed: status {}",
                response.status()
            ));
        }

        let value: Value = response
            .json()
            .map_err(|e| format!("invalid incidents JSON: {e}"))?;

        Ok(value["incidents"].as_array().cloned().unwrap_or_default())
    }

    fn parse_metric_value(latencies: &[u64], percentile: f64) -> f64 {
        if latencies.is_empty() {
            return 0.0;
        }
        let mut sorted = latencies.to_vec();
        sorted.sort_unstable();
        let idx = ((sorted.len() - 1) as f64 * percentile).round() as usize;
        sorted[idx] as f64
    }

    fn process_log(
        &mut self,
        run_id: &str,
        log: &LogRecord,
        detected_anomaly_signals: &mut Vec<Tier2Signal>,
        latencies_micros: &mut Vec<u64>,
        counts: &mut DetectionCounts,
        gt_ids_from_logs: &mut HashSet<String>,
        anomaly_stats: &mut HashMap<String, AnomalyEventStats>,
    ) {
        let start = Instant::now();

        let value = log.metric_value();
        let timestamp: u64 = log.timeUnixNano.parse().unwrap_or(0);
        let entity_hash = xxhash_rust::xxh3::xxh3_64(log.traceId.as_bytes());

        let signal = self
            .profile
            .process_with_hash(timestamp, entity_hash, value);

        latencies_micros.push(start.elapsed().as_micros() as u64);

        if log.isGroundTruthAnomaly {
            counts.gt_events += 1;
            if let Some(id) = &log.anomalyId {
                gt_ids_from_logs.insert(id.clone());
                let stats = anomaly_stats.entry(id.clone()).or_default();
                stats.ground_truth_events += 1;
                if signal.is_anomaly {
                    stats.detected_events += 1;
                }
            }
        }

        match (signal.is_anomaly, log.isGroundTruthAnomaly) {
            (true, true) => counts.tp += 1,
            (true, false) => counts.fp += 1,
            (false, true) => counts.fn_ += 1,
            (false, false) => counts.tn += 1,
        }

        if !signal.is_anomaly {
            return;
        }

        let event_id = format!(
            "{}:{}:{}",
            signal.timestamp, signal.entity_hash, signal.sequence
        );

        let mut attributes = HashMap::new();
        attributes.insert("benchmark_run_id".to_string(), run_id.to_string());
        if let Some(id) = &log.anomalyId {
            attributes.insert("ground_truth_anomaly_id".to_string(), id.clone());
        }

        detected_anomaly_signals.push(Tier2Signal {
            event_id,
            schema_version: 1,
            entity_hash: signal.entity_hash.to_string(),
            timestamp: signal.timestamp.to_string(),
            score: signal.ensemble_score,
            severity: signal.severity as u8,
            primary_detector: signal.attribution.primary_detector,
            detectors_fired: signal.attribution.detectors_fired,
            confidence: signal.confidence,
            detector_scores: signal
                .detector_scores
                .iter()
                .map(|s| s.score as f64)
                .collect(),
            attributes,
        });

        counts.sent += 1;
    }

    fn build_anomaly_breakdown(
        manifests: &[ScheduledAnomalyManifest],
        anomaly_stats: &HashMap<String, AnomalyEventStats>,
    ) -> Vec<AnomalyDetectionBreakdown> {
        manifests
            .iter()
            .map(|manifest| {
                let stats = anomaly_stats
                    .get(&manifest.anomaly_id)
                    .cloned()
                    .unwrap_or_default();
                let missed_events = stats
                    .ground_truth_events
                    .saturating_sub(stats.detected_events);
                let recall = if stats.ground_truth_events > 0 {
                    stats.detected_events as f64 / stats.ground_truth_events as f64
                } else {
                    0.0
                };

                AnomalyDetectionBreakdown {
                    anomaly_id: manifest.anomaly_id.clone(),
                    scenario: manifest.scenario.clone(),
                    scheduled_start_sec: manifest.start_time_sec,
                    scheduled_duration_sec: manifest.duration_sec,
                    ground_truth_events: stats.ground_truth_events,
                    detected_events: stats.detected_events,
                    missed_events,
                    recall,
                }
            })
            .collect()
    }

    pub fn run(
        &mut self,
        cfg: PipelineBenchmarkConfig,
    ) -> Result<PipelineBenchmarkResults, String> {
        let run_id = format!("pipeline_{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));

        let mut engine = SimulationEngine::new();
        engine.start(&cfg.benchmark.base_scenario);

        let mut anomaly_manifest: Vec<ScheduledAnomalyManifest> = Vec::new();
        for anomaly in &cfg.benchmark.anomalies {
            let start_offset_ns = anomaly.start_time_sec * 1_000_000_000;
            let duration_ns = anomaly.duration_sec * 1_000_000_000;
            if let Some(anomaly_id) =
                engine.schedule_anomaly(&anomaly.scenario, start_offset_ns, duration_ns)
            {
                anomaly_manifest.push(ScheduledAnomalyManifest {
                    anomaly_id,
                    scenario: anomaly.scenario.clone(),
                    start_time_sec: anomaly.start_time_sec,
                    duration_sec: anomaly.duration_sec,
                });
            }
        }

        let start = Instant::now();

        let duration_ns = cfg.benchmark.duration_minutes * 60 * 1_000_000_000;
        let tick_ns = cfg.benchmark.tick_ms * 1_000_000;
        let total_ticks = duration_ns / tick_ns;

        let mut counts = DetectionCounts::default();
        let mut latencies_micros: Vec<u64> = Vec::new();
        let mut pending_signals: Vec<Tier2Signal> = Vec::with_capacity(cfg.send_batch_size);
        let mut gt_ids_from_logs: HashSet<String> = HashSet::new();
        let mut anomaly_stats: HashMap<String, AnomalyEventStats> = HashMap::new();

        for _ in 0..total_ticks {
            let batch = engine.tick(tick_ns);

            for resource_log in &batch.logs.resourceLogs {
                for scope_log in &resource_log.scopeLogs {
                    for log in &scope_log.logRecords {
                        counts.total_events += 1;
                        self.process_log(
                            &run_id,
                            log,
                            &mut pending_signals,
                            &mut latencies_micros,
                            &mut counts,
                            &mut gt_ids_from_logs,
                            &mut anomaly_stats,
                        );

                        if pending_signals.len() >= cfg.send_batch_size {
                            self.send_batch(&cfg.tier2_base_url, &pending_signals)?;
                            pending_signals.clear();
                        }
                    }
                }
            }
        }

        if !pending_signals.is_empty() {
            self.send_batch(&cfg.tier2_base_url, &pending_signals)?;
        }

        let adaptive_timeout_secs = cfg
            .drain_timeout_secs
            .max(120)
            .max(counts.sent.saturating_div(50));
        self.wait_for_pipeline_drain(&cfg.tier2_base_url, adaptive_timeout_secs)?;

        let incidents = self.fetch_incidents(&cfg.tier2_base_url, &run_id)?;
        let incident_metrics = compute_incident_metrics(&incidents, &run_id, &gt_ids_from_logs);

        let elapsed = start.elapsed().as_secs_f64();
        let throughput_eps = if elapsed > 0.0 {
            counts.total_events as f64 / elapsed
        } else {
            0.0
        };

        let (detection_precision, detection_recall, detection_f1) =
            calculate_metrics(counts.tp, counts.fp, counts.fn_);
        let anomaly_breakdown =
            Self::build_anomaly_breakdown(&anomaly_manifest, &anomaly_stats);

        Ok(PipelineBenchmarkResults {
            run_id,
            config_name: cfg.benchmark.name,
            total_events: counts.total_events,
            total_ground_truth_anomaly_events: counts.gt_events,
            total_detected_anomalies: counts.tp + counts.fp,
            tier2_events_sent: counts.sent,
            detection_precision,
            detection_recall,
            detection_f1,
            detection_latency_p50_micros: Self::parse_metric_value(&latencies_micros, 0.50),
            detection_latency_p95_micros: Self::parse_metric_value(&latencies_micros, 0.95),
            incident_precision: incident_metrics.precision,
            incident_recall: incident_metrics.recall,
            incident_f1: incident_metrics.f1,
            merge_error_rate: incident_metrics.merge_error_rate,
            split_error_rate: incident_metrics.split_error_rate,
            escalation_quality: incident_metrics.escalation_quality,
            throughput_eps,
            cost_per_10k_events_seconds: if counts.total_events > 0 {
                elapsed * (10_000.0 / counts.total_events as f64)
            } else {
                0.0
            },
            anomaly_breakdown,
        })
    }
}

#[derive(Default)]
struct DetectionCounts {
    total_events: u64,
    gt_events: u64,
    tp: u64,
    fp: u64,
    tn: u64,
    fn_: u64,
    sent: u64,
}

#[derive(Default)]
struct IncidentMetrics {
    precision: f64,
    recall: f64,
    f1: f64,
    merge_error_rate: f64,
    split_error_rate: f64,
    escalation_quality: f64,
}

fn compute_incident_metrics(
    incidents: &[Value],
    run_id: &str,
    true_gt_ids: &HashSet<String>,
) -> IncidentMetrics {
    let mut tp_incidents = 0u64;
    let mut fp_incidents = 0u64;
    let mut covered_gt_ids: HashSet<String> = HashSet::new();
    let mut gt_incident_counts: HashMap<String, u64> = HashMap::new();

    let mut merged_incidents = 0u64;
    let mut escalated_total = 0u64;
    let mut escalated_tp = 0u64;

    for incident in incidents {
        let evidence = incident["evidence"]
            .as_object()
            .cloned()
            .unwrap_or_default();

        let mut run_ids: HashSet<String> = HashSet::new();
        if let Some(v) = evidence.get("benchmark_run_id").and_then(|v| v.as_str()) {
            run_ids.insert(v.to_string());
        }
        if let Some(arr) = evidence.get("benchmark_run_ids").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(v) = item.as_str() {
                    run_ids.insert(v.to_string());
                }
            }
        }

        if !run_ids.contains(run_id) {
            continue;
        }

        let mut gt_ids: HashSet<String> = HashSet::new();
        if let Some(v) = evidence
            .get("ground_truth_anomaly_id")
            .and_then(|v| v.as_str())
        {
            gt_ids.insert(v.to_string());
        }
        if let Some(arr) = evidence
            .get("ground_truth_anomaly_ids")
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(v) = item.as_str() {
                    gt_ids.insert(v.to_string());
                }
            }
        }

        let has_truth = !gt_ids.is_empty();
        if has_truth {
            tp_incidents += 1;
            if gt_ids.len() > 1 {
                merged_incidents += 1;
            }
            for gt_id in gt_ids {
                covered_gt_ids.insert(gt_id.clone());
                *gt_incident_counts.entry(gt_id).or_insert(0) += 1;
            }
        } else {
            fp_incidents += 1;
        }

        if incident["status"].as_str() == Some("escalated") {
            escalated_total += 1;
            if has_truth {
                escalated_tp += 1;
            }
        }
    }

    let (precision, recall, f1) = calculate_metrics(
        tp_incidents,
        fp_incidents,
        true_gt_ids.len().saturating_sub(covered_gt_ids.len()) as u64,
    );

    let split_gt_ids = gt_incident_counts
        .values()
        .filter(|count| **count > 1)
        .count() as u64;

    IncidentMetrics {
        precision,
        recall,
        f1,
        merge_error_rate: if tp_incidents > 0 {
            merged_incidents as f64 / tp_incidents as f64
        } else {
            0.0
        },
        split_error_rate: if !true_gt_ids.is_empty() {
            split_gt_ids as f64 / true_gt_ids.len() as f64
        } else {
            0.0
        },
        escalation_quality: if escalated_total > 0 {
            escalated_tp as f64 / escalated_total as f64
        } else {
            0.0
        },
    }
}

pub fn scenario_by_name(name: &str) -> BenchmarkConfig {
    match name {
        "mixed" => scenarios::mixed_workload(),
        "security" => scenarios::security_audit(),
        "performance" => scenarios::performance_stress(),
        "quick" => scenarios::quick_validation(),
        "throughput" => BenchmarkConfig {
            name: "Pipeline Throughput".to_string(),
            base_scenario: "normal_traffic".to_string(),
            duration_minutes: 2,
            tick_ms: 50,
            anomalies: Vec::<AnomalySpec>::new(),
            batch_size: 0,
        },
        _ => scenarios::quick_validation(),
    }
}
