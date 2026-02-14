//! Tier-2 Forwarder - HTTP Signal Forwarding
//!
//! Forwards anomaly signals from Tier-1 (Rust) to Tier-2 (Bun) via HTTP.
//! Implements bounded async forwarding with retry and backpressure.

use crate::signal::{AnomalySignal, NUM_DETECTORS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub const DEFAULT_TIER2_URL: &str = "http://localhost:3000";
pub const SIGNAL_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Tier1SignalV1 {
    pub event_id: String,
    pub schema_version: u16,
    pub entity_hash: String,
    pub timestamp: u64,
    pub score: f64,
    pub severity: u8,
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
            timestamp: signal.timestamp,
            score: signal.ensemble_score,
            severity: signal.severity as u8,
            primary_detector: signal.attribution.primary_detector,
            detectors_fired: signal.attribution.detectors_fired,
            confidence: signal.confidence,
            detector_scores: signal.detector_scores.map(|s| s.score).to_vec(),
            attributes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalBatch {
    pub signals: Vec<Tier1SignalV1>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tier2Response {
    pub status: String,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
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
            tier2_url: DEFAULT_TIER2_URL.to_string(),
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

    pub fn try_send(&self, signal: AnomalySignal) -> Result<(), AnomalySignal> {
        match self.tx.try_send(signal) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(signal)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                Err(signal)
            }
            Err(mpsc::error::TrySendError::Closed(signal)) => {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
                Err(signal)
            }
        }
    }

    async fn worker(
        mut rx: mpsc::Receiver<AnomalySignal>,
        config: ForwarderConfig,
        stats: Arc<ForwarderStats>,
    ) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .pool_max_idle_per_host(10)
            .build()
            .unwrap();

        let url = format!("{}/tier2/anomalies", config.tier2_url);
        let mut batch: Vec<Tier1SignalV1> = Vec::with_capacity(config.batch_size);
        let mut interval = tokio::time::interval(Duration::from_millis(config.flush_interval_ms));

        info!(url = %url, "Tier-2 forwarder started");

        loop {
            tokio::select! {
                Some(signal) = rx.recv() => {
                    batch.push(Tier1SignalV1::from(signal));
                    if batch.len() >= config.batch_size {
                        Self::flush_batch(&client, &url, &mut batch, &config, &stats).await;
                    }
                }
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        Self::flush_batch(&client, &url, &mut batch, &config, &stats).await;
                    }
                }
                else => break,
            }
        }

        if !batch.is_empty() {
            Self::flush_batch(&client, &url, &mut batch, &config, &stats).await;
        }

        info!("Tier-2 forwarder stopped");
    }

    async fn flush_batch(
        client: &reqwest::Client,
        url: &str,
        batch: &mut Vec<Tier1SignalV1>,
        config: &ForwarderConfig,
        stats: &ForwarderStats,
    ) {
        if batch.is_empty() {
            return;
        }

        let payload = SignalBatch {
            signals: std::mem::take(batch),
        };
        let count = payload.signals.len();

        for attempt in 0..=config.max_retries {
            match client.post(url).json(&payload).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        stats.sent.fetch_add(count as u64, Ordering::Relaxed);
                        stats.batches.fetch_add(1, Ordering::Relaxed);
                        debug!(count, "Forwarded signals to Tier-2");
                        return;
                    } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
                        warn!(attempt, status = %response.status(), "Tier-2 rate limited");
                    } else {
                        warn!(attempt, status = %response.status(), "Tier-2 returned error");
                    }
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Failed to forward to Tier-2");
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

use reqwest::StatusCode;
