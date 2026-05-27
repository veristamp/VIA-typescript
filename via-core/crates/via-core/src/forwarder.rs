//! Tier-2 Forwarder - gRPC Signal Forwarding
//!
//! Forwards anomaly signals from Tier-1 (Rust) to Tier-2 (Bun) via gRPC.
//! Implements bounded async forwarding with retry and backpressure.

use crate::pb::tier2::tier2_service_client::Tier2ServiceClient;
use crate::pb::tier2::{SubmitAnomalyBatchRequest, Tier1Signal};
use crate::signal::AnomalySignal;
use crate::tier2::{TIER1_SCHEMA_VERSION, normalize_tier1_severity, normalize_to_unix_seconds};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub const DEFAULT_TIER2_GRPC_URL: &str = "http://localhost:3002";
pub const SIGNAL_SCHEMA_VERSION: u16 = TIER1_SCHEMA_VERSION;
const DEDUPE_WINDOW_SEC: u64 = 900;

#[derive(Debug, Clone, Serialize)]
pub struct Tier1SignalV1 {
    pub event_id: String,
    pub schema_version: u16,
    pub entity_hash: String,
    pub timestamp: u64,
    pub score: f64,
    pub severity: f64,
    pub primary_detector: u8,
    pub detectors_fired: u8,
    pub confidence: f64,
    pub detector_scores: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
}

impl From<AnomalySignal> for Tier1SignalV1 {
    fn from(signal: AnomalySignal) -> Self {
        let event_id = format!(
            "{:016x}-{}-{}",
            signal.entity_hash, signal.timestamp, signal.sequence
        );

        Self {
            event_id,
            schema_version: SIGNAL_SCHEMA_VERSION,
            entity_hash: signal.entity_hash.to_string(),
            timestamp: normalize_to_unix_seconds(signal.timestamp),
            score: signal.ensemble_score,
            severity: normalize_tier1_severity(signal.severity as u8 as f64, SIGNAL_SCHEMA_VERSION),
            primary_detector: signal.attribution.primary_detector,
            detectors_fired: signal.attribution.detectors_fired,
            confidence: signal.confidence,
            detector_scores: signal.detector_scores.map(|s| s.score).to_vec(),
            attributes: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForwarderConfig {
    pub tier2_url: String,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
    pub channel_capacity: usize,
    pub timeout_ms: u64,
}

impl Default for ForwarderConfig {
    fn default() -> Self {
        Self {
            tier2_url: DEFAULT_TIER2_GRPC_URL.to_string(),
            batch_size: 100,
            flush_interval_ms: 1000,
            max_retries: 3,
            retry_base_delay_ms: 100,
            channel_capacity: 10_000,
            timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Default)]
pub struct ForwarderStats {
    pub sent: AtomicU64,
    pub failed: AtomicU64,
    pub retried: AtomicU64,
    pub dropped: AtomicU64,
    pub batches: AtomicU64,
}

pub struct Tier2Forwarder {
    tx: mpsc::Sender<AnomalySignal>,
    stats: Arc<ForwarderStats>,
}

impl Tier2Forwarder {
    pub fn new(config: ForwarderConfig) -> Self {
        let (tx, rx) = mpsc::channel(config.channel_capacity);
        let stats = Arc::new(ForwarderStats::default());
        let stats_clone = stats.clone();

        tokio::spawn(async move {
            Self::worker(rx, config, stats_clone).await;
        });

        Self { tx, stats }
    }

    pub fn stats(&self) -> &ForwarderStats {
        &self.stats
    }

    pub fn try_send(&self, signal: AnomalySignal) -> Result<(), Box<AnomalySignal>> {
        match self.tx.try_send(signal) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(signal)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                Err(Box::new(signal))
            }
            Err(mpsc::error::TrySendError::Closed(signal)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                Err(Box::new(signal))
            }
        }
    }

    async fn worker(
        mut rx: mpsc::Receiver<AnomalySignal>,
        config: ForwarderConfig,
        stats: Arc<ForwarderStats>,
    ) {
        let mut batch: Vec<Tier1SignalV1> = Vec::with_capacity(config.batch_size);
        let mut dedupe: HashMap<String, u64> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_millis(config.flush_interval_ms));
        let mut client: Option<Tier2ServiceClient<tonic::transport::Channel>> = None;

        info!(url = %config.tier2_url, "Tier-2 gRPC forwarder started");

        loop {
            tokio::select! {
                Some(signal) = rx.recv() => {
                    let payload = Tier1SignalV1::from(signal);
                    Self::cleanup_dedupe(&mut dedupe);
                    if dedupe.contains_key(&payload.event_id) {
                        stats.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    dedupe.insert(
                        payload.event_id.clone(),
                        now_unix().saturating_add(DEDUPE_WINDOW_SEC),
                    );
                    batch.push(payload);
                    if batch.len() >= config.batch_size {
                        Self::flush_batch(&mut client, &mut batch, &config, &stats).await;
                    }
                }
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        Self::flush_batch(&mut client, &mut batch, &config, &stats).await;
                    }
                }
                else => break,
            }
        }

        if !batch.is_empty() {
            Self::flush_batch(&mut client, &mut batch, &config, &stats).await;
        }

        info!("Tier-2 forwarder stopped");
    }

    fn cleanup_dedupe(dedupe: &mut HashMap<String, u64>) {
        let now = now_unix();
        dedupe.retain(|_, expires_at| *expires_at > now);
    }

    async fn flush_batch(
        client: &mut Option<Tier2ServiceClient<tonic::transport::Channel>>,
        batch: &mut Vec<Tier1SignalV1>,
        config: &ForwarderConfig,
        stats: &ForwarderStats,
    ) {
        if batch.is_empty() {
            return;
        }

        let payload = std::mem::take(batch);
        let count = payload.len();

        for attempt in 0..=config.max_retries {
            if client.is_none() {
                match Tier2ServiceClient::connect(config.tier2_url.clone()).await {
                    Ok(connected) => {
                        *client = Some(connected);
                    }
                    Err(error) => {
                        warn!(attempt, error = %error, "Failed to connect to Tier-2 gRPC");
                    }
                }
            }

            if let Some(connected) = client.as_mut() {
                let request = SubmitAnomalyBatchRequest {
                    signals: payload.iter().map(Tier1Signal::from).collect(),
                };
                match tokio::time::timeout(
                    Duration::from_millis(config.timeout_ms),
                    connected.submit_anomaly_batch(request),
                )
                .await
                {
                    Ok(Ok(_response)) => {
                        stats.sent.fetch_add(count as u64, Ordering::Relaxed);
                        stats.batches.fetch_add(1, Ordering::Relaxed);
                        debug!(count, "Forwarded signals to Tier-2 over gRPC");
                        return;
                    }
                    Ok(Err(error)) => {
                        warn!(attempt, error = %error, "Tier-2 gRPC returned error");
                        *client = None;
                    }
                    Err(error) => {
                        warn!(attempt, error = %error, "Tier-2 gRPC request timed out");
                        *client = None;
                    }
                }
            }

            if attempt < config.max_retries {
                stats.retried.fetch_add(1, Ordering::Relaxed);
                let delay = config.retry_base_delay_ms * (1 << attempt);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }

        stats.failed.fetch_add(count as u64, Ordering::Relaxed);
        error!(count, "Dropped signals after max retries");
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl From<&Tier1SignalV1> for Tier1Signal {
    fn from(signal: &Tier1SignalV1) -> Self {
        Self {
            event_id: signal.event_id.clone(),
            schema_version: signal.schema_version as u32,
            entity_hash: signal.entity_hash.clone(),
            timestamp: signal.timestamp,
            score: signal.score,
            severity: signal.severity,
            primary_detector: signal.primary_detector as u32,
            detectors_fired: signal.detectors_fired as u32,
            confidence: signal.confidence,
            detector_scores: signal.detector_scores.clone(),
            attributes: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{NUM_DETECTORS, Severity};

    #[test]
    fn converts_anomaly_signal_to_normalized_tier2_payload() {
        let signal = AnomalySignal {
            entity_hash: 42,
            timestamp: 1_738_000_000_000_000_000,
            sequence: 7,
            is_anomaly: true,
            severity: Severity::High,
            ensemble_score: 0.8,
            confidence: 0.9,
            detector_scores: Default::default(),
            detector_weights: [0.1; NUM_DETECTORS],
            attribution: Default::default(),
            baseline: Default::default(),
            raw_value: 123.0,
        };

        let payload = Tier1SignalV1::from(signal);

        assert_eq!(payload.schema_version, SIGNAL_SCHEMA_VERSION);
        assert_eq!(payload.entity_hash, "42");
        assert_eq!(payload.timestamp, 1_738_000_000);
        assert_eq!(payload.severity, 0.75);
    }
}
