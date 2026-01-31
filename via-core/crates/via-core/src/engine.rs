//! VIA-Core Detection Engine v2
//!
//! Two-stage pipeline architecture:
//! 1. Detection Stage: Run all 10 detectors independently
//! 2. Decision Stage: Combine with AdaptiveEnsemble, produce rich signals
//!
//! This engine produces `AnomalySignal` with full detector breakdown and attribution.

use crate::algo::{
    AdaptiveThreshold,
    adaptive_ensemble::{AdaptiveEnsemble, DetectorOutput},
    adaptive_threshold::presets,
    behavioral_fingerprint::BehavioralFingerprintDetector,
    drift_detector::{DriftType, EnsembleDriftDetector},
    enhanced_cusum::EnhancedCUSUM,
    ewma::EWMA,
    histogram::FadingHistogram,
    hll::HyperLogLog,
    holtwinters::HoltWinters,
    multi_scale::MultiScaleDetector,
    rrcf::RRCFDetector,
    spectral_residual::SpectralResidual,
};
use crate::checkpoint::{CheckpointError, Checkpointable, EnsembleCheckpoint};
use crate::feedback::{FeedbackEvent, LearningUpdate};
use crate::signal::{
    AnomalySignal, Attribution, BaselineSummary, DetectorId, DetectorScore, NUM_DETECTORS, Severity,
};

// ============================================================================
// CORE ABSTRACTIONS
// ============================================================================

/// Context passed to every detector for every event
#[derive(Debug, Clone, Copy)]
pub struct SignalContext {
    pub timestamp: u64,
    pub unique_id_hash: u64,
    pub value: f64,
    pub is_warmup: bool,
    pub sequence: u64,
}

/// Internal detection result from a single detector
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub score: f64,
    pub weight: f64,
    pub signal_type: u8,
    pub expected: f64,
    pub confidence: f64,
    pub reason: String,
}

/// Trait for all detectors
pub trait Detector: Send + Sync {
    fn name(&self) -> &str;
    fn id(&self) -> DetectorId;
    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult>;
    fn get_stats(&self) -> String {
        String::new()
    }
}

// ============================================================================
// DETECTOR IMPLEMENTATIONS (Refactored to return DetectorId)
// ============================================================================

/// Volume Detector (Holt-Winters + Adaptive Threshold)
pub struct VolumeDetectorV2 {
    hw: HoltWinters,
    rate_estimator: EWMA,
    adaptive_threshold: AdaptiveThreshold,
    last_timestamp: u64,
    warmup_count: usize,
}

impl VolumeDetectorV2 {
    pub fn new(alpha: f64, beta: f64, gamma: f64, period: usize) -> Self {
        Self {
            hw: HoltWinters::new(alpha, beta, gamma, period),
            rate_estimator: EWMA::new(50.0),
            adaptive_threshold: presets::volume_threshold(),
            last_timestamp: 0,
            warmup_count: 0,
        }
    }
}

impl Detector for VolumeDetectorV2 {
    fn name(&self) -> &str {
        "Volume/RPS-V2"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Volume
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        let delta_ns = ctx.timestamp.saturating_sub(self.last_timestamp).max(1);
        let delta_sec = delta_ns as f64 / 1_000_000_000.0;
        let instant_rps = if delta_sec > 0.0 {
            1.0 / delta_sec
        } else {
            0.0
        };
        let smoothed_rps = self.rate_estimator.update(instant_rps);

        self.last_timestamp = ctx.timestamp;
        self.warmup_count += 1;

        let (predicted, deviation) = self.hw.update(smoothed_rps);

        if ctx.is_warmup || self.warmup_count < 100 {
            return None;
        }

        let _ = self.adaptive_threshold.update(deviation.abs());
        let score = self.adaptive_threshold.anomaly_score(deviation.abs());

        let prediction_error = deviation.abs() / predicted.max(1.0);
        let confidence = if prediction_error < 0.1 {
            0.9
        } else if prediction_error < 0.3 {
            0.7
        } else {
            0.5
        };

        if score > 0.0 {
            Some(DetectionResult {
                score,
                weight: 1.0,
                signal_type: DetectorId::Volume as u8,
                expected: predicted,
                confidence,
                reason: format!(
                    "Volume {}: expected {:.1} RPS, observed {:.1} RPS",
                    if deviation > 0.0 { "spike" } else { "drop" },
                    predicted,
                    smoothed_rps
                ),
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (mean, std, thresh, count) = self.adaptive_threshold.get_stats();
        format!(
            "VolumeV2: μ={:.2}, σ={:.2}, thresh={:.2}, n={}",
            mean, std, thresh, count
        )
    }
}

/// Distribution Detector (Fading Histogram)
pub struct DistributionDetectorV2 {
    hist: FadingHistogram,
    adaptive_threshold: AdaptiveThreshold,
}

impl DistributionDetectorV2 {
    pub fn new(bins: usize, min: f64, max: f64, decay: f64) -> Self {
        Self {
            hist: FadingHistogram::new(bins, min, max, decay),
            adaptive_threshold: presets::distribution_threshold(),
        }
    }
}

impl Detector for DistributionDetectorV2 {
    fn name(&self) -> &str {
        "Distribution/Value-V2"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Distribution
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let anomaly_likelihood = self.hist.update(ctx.value);
        let _ = self.adaptive_threshold.update(anomaly_likelihood);
        let score = self.adaptive_threshold.anomaly_score(anomaly_likelihood);

        let confidence = if anomaly_likelihood > 50.0 {
            0.95
        } else if anomaly_likelihood > 20.0 {
            0.8
        } else if anomaly_likelihood > 10.0 {
            0.6
        } else {
            0.4
        };

        if score > 0.0 {
            Some(DetectionResult {
                score,
                weight: 0.8,
                signal_type: DetectorId::Distribution as u8,
                expected: 0.0,
                confidence,
                reason: format!(
                    "Distribution shift: value {:.2} has rarity score {:.1}",
                    ctx.value, anomaly_likelihood
                ),
            })
        } else {
            None
        }
    }
}

/// Cardinality Detector (HLL Velocity)
pub struct CardinalityDetectorV2 {
    hll: HyperLogLog,
    velocity_tracker: EWMA,
    adaptive_threshold: AdaptiveThreshold,
    last_count: f64,
    last_velocity: f64,
}

impl CardinalityDetectorV2 {
    pub fn new() -> Self {
        Self {
            hll: HyperLogLog::new(12),
            velocity_tracker: EWMA::new(100.0),
            adaptive_threshold: presets::cardinality_threshold(),
            last_count: 0.0,
            last_velocity: 0.0,
        }
    }
}

impl Default for CardinalityDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for CardinalityDetectorV2 {
    fn name(&self) -> &str {
        "Cardinality/Velocity-V2"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Cardinality
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.hll.add_hash(ctx.unique_id_hash);
        let current_count = self.hll.count();
        let delta = current_count - self.last_count;
        self.last_count = current_count;

        let velocity = if delta > 0.0 { delta } else { 0.0 };
        self.last_velocity = self.velocity_tracker.update(velocity);

        let _ = self.adaptive_threshold.update(velocity);
        let score = self.adaptive_threshold.anomaly_score(velocity);

        let confidence = if velocity > self.last_velocity * 10.0 {
            0.95
        } else {
            0.85
        };

        if score > 0.0 {
            Some(DetectionResult {
                score,
                weight: 1.2,
                signal_type: DetectorId::Cardinality as u8,
                expected: self.last_velocity,
                confidence,
                reason: format!(
                    "New unique entities: {:.0} new (velocity: {:.1}/event)",
                    delta, velocity
                ),
            })
        } else {
            None
        }
    }
}

/// Burst Detector (Enhanced CUSUM)
pub struct BurstDetectorV2 {
    cusum: EnhancedCUSUM,
    last_timestamp: u64,
}

impl BurstDetectorV2 {
    pub fn new() -> Self {
        Self {
            cusum: EnhancedCUSUM::with_options(50.0, 10.0, 4.0, 5, true, 0.5),
            last_timestamp: 0,
        }
    }
}

impl Default for BurstDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for BurstDetectorV2 {
    fn name(&self) -> &str {
        "Burst/IAT-V2"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Burst
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        let delta_ns = ctx.timestamp.saturating_sub(self.last_timestamp);
        let delta_ms = delta_ns as f64 / 1_000_000.0;
        self.last_timestamp = ctx.timestamp;

        let burst_indicator = 100.0 - delta_ms;
        let alarm = self.cusum.update(burst_indicator);

        if alarm {
            let severity = self.cusum.alarm_severity;
            let alarm_type = self.cusum.alarm_type;

            Some(DetectionResult {
                score: severity,
                weight: 0.6,
                signal_type: DetectorId::Burst as u8,
                expected: 50.0,
                confidence: 0.75,
                reason: format!(
                    "Burst detected: IAT {:.1}ms (type: {})",
                    delta_ms,
                    if alarm_type > 0 {
                        "clustering"
                    } else {
                        "dispersion"
                    }
                ),
            })
        } else {
            None
        }
    }
}

/// Spectral Detector (FFT Residual)
pub struct SpectralDetector {
    spectral: SpectralResidual,
    last_values: Vec<f64>,
}

impl SpectralDetector {
    pub fn new() -> Self {
        Self {
            spectral: SpectralResidual::new(24, 0.6),
            last_values: Vec::with_capacity(5),
        }
    }
}

impl Default for SpectralDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SpectralDetector {
    fn name(&self) -> &str {
        "Spectral/FFT"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Spectral
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.last_values.push(ctx.value);
        if self.last_values.len() > 5 {
            self.last_values.remove(0);
        }

        let (score, is_anomaly) = self.spectral.update(ctx.value);

        if is_anomaly && score > 0.3 {
            let trend = if self.last_values.len() >= 2 {
                let first = self.last_values.first().unwrap_or(&ctx.value);
                let last = self.last_values.last().unwrap_or(&ctx.value);
                if last > first { "spike" } else { "drop" }
            } else {
                "anomaly"
            };

            Some(DetectionResult {
                score,
                weight: 1.0,
                signal_type: DetectorId::Spectral as u8,
                expected: 0.0,
                confidence: 0.85,
                reason: format!("Spectral anomaly: {} (FFT residual: {:.2})", trend, score),
            })
        } else {
            None
        }
    }
}

/// Change Point Detector (Trend CUSUM)
pub struct ChangePointDetector {
    cusum: EnhancedCUSUM,
    trend_ewma: EWMA,
    last_value: f64,
}

impl ChangePointDetector {
    pub fn new() -> Self {
        Self {
            cusum: EnhancedCUSUM::with_options(0.0, 0.5, 4.0, 8, true, 0.5),
            trend_ewma: EWMA::new(100.0),
            last_value: 0.0,
        }
    }
}

impl Default for ChangePointDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ChangePointDetector {
    fn name(&self) -> &str {
        "ChangePoint/Trend"
    }

    fn id(&self) -> DetectorId {
        DetectorId::ChangePoint
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_value == 0.0 {
            self.last_value = ctx.value;
            return None;
        }

        let change = ctx.value - self.last_value;
        self.last_value = ctx.value;

        let smoothed_change = self.trend_ewma.update(change);
        let alarm = self.cusum.update(smoothed_change);

        if alarm {
            let severity = self.cusum.alarm_severity;
            let alarm_type = self.cusum.alarm_type;

            Some(DetectionResult {
                score: severity,
                weight: 0.9,
                signal_type: DetectorId::ChangePoint as u8,
                expected: 0.0,
                confidence: 0.8,
                reason: format!(
                    "Trend change: sustained {} (severity: {:.0}%)",
                    if alarm_type > 0 {
                        "increase"
                    } else {
                        "decrease"
                    },
                    severity * 100.0
                ),
            })
        } else {
            None
        }
    }
}

/// RRCF Detector (Random Cut Forest)
pub struct RRCFDetectorV2 {
    rrcf: RRCFDetector,
    warmup_count: usize,
}

impl RRCFDetectorV2 {
    pub fn new() -> Self {
        Self {
            rrcf: RRCFDetector::new_univariate(10),
            warmup_count: 0,
        }
    }
}

impl Default for RRCFDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for RRCFDetectorV2 {
    fn name(&self) -> &str {
        "RRCF/Multivariate"
    }

    fn id(&self) -> DetectorId {
        DetectorId::RRCF
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let (score, is_anomaly) = self.rrcf.update(ctx.value);
        self.warmup_count += 1;

        if self.warmup_count > 20 && is_anomaly && score > 0.5 {
            Some(DetectionResult {
                score,
                weight: 1.1,
                signal_type: DetectorId::RRCF as u8,
                expected: 0.0,
                confidence: (score * 0.9).min(0.95),
                reason: format!("RRCF anomaly: co-displacement score {:.2}", score),
            })
        } else {
            None
        }
    }
}

/// Multi-Scale Detector
pub struct MultiScaleDetectorV2 {
    multi_scale: MultiScaleDetector,
}

impl MultiScaleDetectorV2 {
    pub fn new() -> Self {
        Self {
            multi_scale: MultiScaleDetector::new(),
        }
    }
}

impl Default for MultiScaleDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for MultiScaleDetectorV2 {
    fn name(&self) -> &str {
        "MultiScale/Temporal"
    }

    fn id(&self) -> DetectorId {
        DetectorId::MultiScale
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let result = self.multi_scale.update(ctx.value, ctx.timestamp);

        if result.is_anomaly && result.combined_score > 0.5 {
            let scales_triggered = result
                .active_scales
                .iter()
                .filter(|(_, s, _)| *s > 0.5)
                .count();

            Some(DetectionResult {
                score: result.combined_score,
                weight: 1.0,
                signal_type: DetectorId::MultiScale as u8,
                expected: 0.0,
                confidence: 0.75 + (scales_triggered as f64 * 0.05).min(0.2),
                reason: format!(
                    "Multi-scale anomaly: {} resolution(s) triggered",
                    scales_triggered
                ),
            })
        } else {
            None
        }
    }
}

/// Behavioral Fingerprint Detector
pub struct BehavioralFingerprintDetectorV2 {
    behavioral: BehavioralFingerprintDetector,
}

impl BehavioralFingerprintDetectorV2 {
    pub fn new() -> Self {
        Self {
            behavioral: BehavioralFingerprintDetector::new(1000),
        }
    }
}

impl Default for BehavioralFingerprintDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for BehavioralFingerprintDetectorV2 {
    fn name(&self) -> &str {
        "Behavioral/Fingerprint"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Behavioral
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let (score, is_anomaly, reason) = self.behavioral.process(
            ctx.unique_id_hash,
            ctx.timestamp,
            ctx.value.abs(),
            ctx.unique_id_hash.wrapping_mul(31),
        );

        if is_anomaly && score > 0.6 {
            Some(DetectionResult {
                score,
                weight: 1.2,
                signal_type: DetectorId::Behavioral as u8,
                expected: 0.0,
                confidence: (score * 0.85).min(0.95),
                reason,
            })
        } else {
            None
        }
    }
}

/// Drift Detector (Concept Drift)
pub struct DriftDetectorV2 {
    drift: EnsembleDriftDetector,
    sample_count: u64,
}

impl DriftDetectorV2 {
    pub fn new() -> Self {
        Self {
            drift: EnsembleDriftDetector::new(),
            sample_count: 0,
        }
    }
}

impl Default for DriftDetectorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for DriftDetectorV2 {
    fn name(&self) -> &str {
        "Drift/Concept"
    }

    fn id(&self) -> DetectorId {
        DetectorId::Drift
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.sample_count += 1;

        if self.sample_count < 100 {
            return None;
        }

        let (drift_type, severity) = self.drift.update(ctx.value);

        if drift_type != DriftType::None {
            let drift_name = match drift_type {
                DriftType::Sudden => "sudden shift",
                DriftType::Gradual => "gradual drift",
                DriftType::Incremental => "incremental change",
                DriftType::Seasonal => "seasonal pattern",
                DriftType::None => "unknown",
            };

            Some(DetectionResult {
                score: severity,
                weight: 0.9,
                signal_type: DetectorId::Drift as u8,
                expected: 0.0,
                confidence: 0.7 + (severity * 0.25),
                reason: format!(
                    "Concept drift: {} (severity: {:.0}%)",
                    drift_name,
                    severity * 100.0
                ),
            })
        } else {
            None
        }
    }
}

// ============================================================================
// ENHANCED ANOMALY PROFILE WITH ADAPTIVE ENSEMBLE
// ============================================================================

/// Configuration for the anomaly profile
#[derive(Debug, Clone)]
pub struct ProfileConfig {
    pub hw_alpha: f64,
    pub hw_beta: f64,
    pub hw_gamma: f64,
    pub period: usize,
    pub hist_bins: usize,
    pub min_val: f64,
    pub max_val: f64,
    pub hist_decay: f64,
    pub confidence_threshold: f64,
    pub warmup_events: usize,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            hw_alpha: 0.3,
            hw_beta: 0.1,
            hw_gamma: 0.1,
            period: 24,
            hist_bins: 50,
            min_val: 0.0,
            max_val: 10000.0,
            hist_decay: 0.999,
            confidence_threshold: 0.5,
            warmup_events: 100,
        }
    }
}

/// Enhanced Anomaly Profile with Adaptive Ensemble
pub struct AnomalyProfile {
    /// All 10 detectors
    detectors: Vec<Box<dyn Detector>>,
    /// Adaptive ensemble for weight learning
    ensemble: AdaptiveEnsemble,
    /// Event counter
    event_count: u64,
    /// Configuration
    config: ProfileConfig,
    /// Baseline tracking
    value_sum: f64,
    value_sum_sq: f64,
    last_timestamp: u64,
    frequency_ewma: EWMA,
}

impl AnomalyProfile {
    /// Create with default configuration
    pub fn default() -> Self {
        Self::with_config(ProfileConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: ProfileConfig) -> Self {
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(VolumeDetectorV2::new(
                config.hw_alpha,
                config.hw_beta,
                config.hw_gamma,
                config.period,
            )),
            Box::new(DistributionDetectorV2::new(
                config.hist_bins,
                config.min_val,
                config.max_val,
                config.hist_decay,
            )),
            Box::new(CardinalityDetectorV2::new()),
            Box::new(BurstDetectorV2::new()),
            Box::new(SpectralDetector::new()),
            Box::new(ChangePointDetector::new()),
            Box::new(RRCFDetectorV2::new()),
            Box::new(MultiScaleDetectorV2::new()),
            Box::new(BehavioralFingerprintDetectorV2::new()),
            Box::new(DriftDetectorV2::new()),
        ];

        let detector_names: Vec<String> = detectors.iter().map(|d| d.name().to_string()).collect();
        let ensemble = AdaptiveEnsemble::default_ensemble(detector_names);

        Self {
            detectors,
            ensemble,
            event_count: 0,
            config,
            value_sum: 0.0,
            value_sum_sq: 0.0,
            last_timestamp: 0,
            frequency_ewma: EWMA::new(100.0),
        }
    }

    /// Legacy constructor for backward compatibility
    pub fn new(
        hw_alpha: f64,
        hw_beta: f64,
        hw_gamma: f64,
        period: usize,
        hist_bins: usize,
        min_val: f64,
        max_val: f64,
        hist_decay: f64,
    ) -> Self {
        Self::with_config(ProfileConfig {
            hw_alpha,
            hw_beta,
            hw_gamma,
            period,
            hist_bins,
            min_val,
            max_val,
            hist_decay,
            ..Default::default()
        })
    }

    /// Process an event and return a rich signal (main hot path)
    pub fn process(&mut self, timestamp: u64, unique_id: &str, value: f64) -> AnomalySignal {
        let hash = xxhash_rust::xxh3::xxh3_64(unique_id.as_bytes());
        self.process_with_hash(timestamp, hash, value)
    }

    /// Zero-allocation hot path
    pub fn process_with_hash(
        &mut self,
        timestamp: u64,
        unique_id_hash: u64,
        value: f64,
    ) -> AnomalySignal {
        self.event_count += 1;

        // Update baseline tracking
        self.value_sum += value;
        self.value_sum_sq += value * value;

        // Track frequency
        if self.last_timestamp > 0 {
            let delta_ns = timestamp.saturating_sub(self.last_timestamp);
            let delta_sec = delta_ns as f64 / 1_000_000_000.0;
            if delta_sec > 0.0 {
                self.frequency_ewma.update(1.0 / delta_sec);
            }
        }
        self.last_timestamp = timestamp;

        let is_warmup = self.event_count < self.config.warmup_events as u64;

        let ctx = SignalContext {
            timestamp,
            unique_id_hash,
            value,
            is_warmup,
            sequence: self.event_count,
        };

        // === STAGE 1: Run all detectors ===
        let mut detector_outputs: Vec<DetectorOutput> = Vec::with_capacity(NUM_DETECTORS);
        let mut detector_scores = [DetectorScore::default(); NUM_DETECTORS];

        for detector in self.detectors.iter_mut() {
            let detector_id = detector.id() as usize;

            if let Some(result) = detector.update(&ctx) {
                detector_scores[detector_id] = DetectorScore::new(
                    result.score,
                    result.confidence,
                    true,
                    result.expected,
                    value,
                );

                detector_outputs.push(DetectorOutput {
                    detector_id,
                    detector_name: detector.name().to_string(),
                    score: result.score,
                    confidence: result.confidence,
                    signal_type: result.signal_type,
                });
            } else {
                detector_outputs.push(DetectorOutput {
                    detector_id,
                    detector_name: detector.name().to_string(),
                    score: 0.0,
                    confidence: 1.0,
                    signal_type: 0,
                });
            }
        }

        // === STAGE 2: Combine with AdaptiveEnsemble ===
        let (ensemble_score, ensemble_confidence, weights) =
            self.ensemble.combine(&detector_outputs);

        // Convert weights to fixed array
        let mut weight_array = [0.1f32; NUM_DETECTORS];
        for (i, w) in weights.iter().enumerate().take(NUM_DETECTORS) {
            weight_array[i] = *w as f32;
        }

        // Compute baseline summary
        let n = self.event_count as f64;
        let avg = self.value_sum / n.max(1.0);
        let variance = (self.value_sum_sq / n.max(1.0)) - (avg * avg);
        let std = variance.max(0.0).sqrt();

        let baseline = BaselineSummary {
            avg_value: avg as f32,
            std_value: std as f32,
            avg_frequency: self.frequency_ewma.get_value() as f32,
            profile_age: self.event_count as u32,
            is_warmup,
        };

        // Compute attribution
        let weights_f64: [f64; NUM_DETECTORS] = {
            let mut arr = [0.0; NUM_DETECTORS];
            for (i, w) in weight_array.iter().enumerate() {
                arr[i] = *w as f64;
            }
            arr
        };
        let attribution = Attribution::compute(&detector_scores, &weights_f64);

        // Build the signal
        let severity = Severity::from_score(ensemble_score);
        let is_anomaly =
            ensemble_score >= 0.4 && ensemble_confidence >= self.config.confidence_threshold;

        AnomalySignal {
            entity_hash: unique_id_hash,
            timestamp,
            sequence: self.event_count,
            is_anomaly,
            severity,
            ensemble_score,
            confidence: ensemble_confidence,
            detector_scores,
            detector_weights: weight_array,
            attribution,
            baseline,
            raw_value: value,
        }
    }

    /// Apply feedback to update ensemble weights
    pub fn apply_feedback(&mut self, events: &[FeedbackEvent]) {
        if events.is_empty() {
            return;
        }

        let update = LearningUpdate::from_batch(events);

        if !update.is_significant() {
            return;
        }

        // Create detector outputs for weight update
        for event in events {
            let outputs: Vec<DetectorOutput> = event
                .detector_scores
                .iter()
                .enumerate()
                .map(|(i, &score)| {
                    let name = DetectorId::from_u8(i as u8)
                        .map(|d| d.name())
                        .unwrap_or("Unknown")
                        .to_string();

                    DetectorOutput {
                        detector_id: i,
                        detector_name: name,
                        score: score as f64,
                        confidence: 0.8,
                        signal_type: i as u8,
                    }
                })
                .collect();

            self.ensemble.update_with_feedback(
                &outputs,
                event.original_decision,
                event.was_true_positive,
            );
        }
    }

    /// Get current ensemble weights
    pub fn get_weights(&self) -> Vec<f64> {
        self.ensemble
            .get_weights()
            .into_iter()
            .map(|(_, w)| w)
            .collect()
    }

    /// Get detector statistics
    pub fn get_detector_stats(&self) -> Vec<(String, String)> {
        self.detectors
            .iter()
            .map(|d| (d.name().to_string(), d.get_stats()))
            .collect()
    }

    /// Reset the profile
    pub fn reset(&mut self) {
        self.event_count = 0;
        self.value_sum = 0.0;
        self.value_sum_sq = 0.0;
        self.last_timestamp = 0;
        self.ensemble.reset();
    }

    /// Get event count
    pub fn event_count(&self) -> u64 {
        self.event_count
    }
}

impl Checkpointable for AnomalyProfile {
    fn to_checkpoint(&self) -> Vec<u8> {
        // Serialize ensemble state
        let weights = self.get_weights();
        let checkpoint = EnsembleCheckpoint {
            weights: {
                let mut arr = [0.1; NUM_DETECTORS];
                for (i, w) in weights.iter().enumerate().take(NUM_DETECTORS) {
                    arr[i] = *w;
                }
                arr
            },
            alpha: [1.0; NUM_DETECTORS], // TODO: expose from AdaptiveEnsemble
            beta: [1.0; NUM_DETECTORS],
            total_samples: self.event_count,
        };

        bincode::serialize(&checkpoint).unwrap_or_default()
    }

    fn from_checkpoint(data: &[u8]) -> Result<Self, CheckpointError> {
        let checkpoint: EnsembleCheckpoint = bincode::deserialize(data)
            .map_err(|e| CheckpointError::DeserializationFailed(e.to_string()))?;

        let mut profile = AnomalyProfile::default();
        profile.event_count = checkpoint.total_samples;
        // TODO: Restore ensemble weights

        Ok(profile)
    }
}

// ============================================================================
// LEGACY COMPATIBILITY: AnomalyResult (deprecated, use AnomalySignal)
// ============================================================================

/// Legacy result struct for backward compatibility
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub severity: u8,
    pub anomaly_score: f64,
    pub signal_type: u8,
    pub expected: f64,
    pub actual: f64,
    pub confidence: f64,
}

impl From<AnomalySignal> for AnomalyResult {
    fn from(signal: AnomalySignal) -> Self {
        Self {
            is_anomaly: signal.is_anomaly,
            severity: signal.severity as u8,
            anomaly_score: signal.ensemble_score,
            signal_type: signal.attribution.primary_detector,
            expected: signal.baseline.avg_value as f64,
            actual: signal.raw_value,
            confidence: signal.confidence,
        }
    }
}

impl AnomalyProfile {
    /// Legacy method returning minimal result
    pub fn process_legacy(&mut self, timestamp: u64, unique_id: &str, value: f64) -> AnomalyResult {
        self.process(timestamp, unique_id, value).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = AnomalyProfile::default();
        assert_eq!(profile.detectors.len(), 10);
        assert_eq!(profile.event_count, 0);
    }

    #[test]
    fn test_signal_generation() {
        let mut profile = AnomalyProfile::default();

        // Process some events
        for i in 0..200 {
            let signal = profile.process_with_hash(i * 1_000_000, 12345, 100.0 + (i as f64 * 0.1));
            assert_eq!(signal.entity_hash, 12345);
            assert_eq!(signal.sequence, i + 1);
        }

        // Check baseline is being tracked
        assert!(profile.event_count() > 0);
    }

    #[test]
    fn test_anomaly_detection() {
        let mut profile = AnomalyProfile::default();

        // Warmup with normal values
        for i in 0..150 {
            profile.process_with_hash(i * 50_000_000, 12345, 100.0);
        }

        // Inject anomaly
        let signal = profile.process_with_hash(150 * 50_000_000, 12345, 10000.0);

        // Should detect something (distribution shift at minimum)
        // Note: Detection depends on warmup and thresholds
        assert!(signal.detector_scores[DetectorId::Distribution as usize].score > 0.0);
    }

    #[test]
    fn test_legacy_compatibility() {
        let mut profile = AnomalyProfile::default();
        let result = profile.process_legacy(1000000, "user123", 100.0);

        assert!(!result.is_anomaly); // Warmup period
        assert_eq!(result.actual, 100.0);
    }

    #[test]
    fn test_checkpointable() {
        let profile = AnomalyProfile::default();
        let checkpoint = profile.to_checkpoint();

        assert!(!checkpoint.is_empty());

        let restored = AnomalyProfile::from_checkpoint(&checkpoint).unwrap();
        assert_eq!(restored.detectors.len(), 10);
    }
}
