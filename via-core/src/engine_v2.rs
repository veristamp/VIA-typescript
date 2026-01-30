use crate::algo::{
    adaptive_threshold::{presets, AdaptiveThreshold},
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

// --- Core Abstractions ---

/// Context passed to every detector for every event
pub struct SignalContext {
    pub timestamp: u64,
    pub unique_id_hash: u64, // CHANGED: Pre-hashed ID for zero-allocation
    pub value: f64,
    pub is_warmup: bool,
}

/// Enhanced DetectionResult with confidence and reasoning
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub score: f64,      // 0.0 (Normal) to 1.0 (Critical)
    pub weight: f64,     // How much this detector matters (0.0 to 1.0)
    pub signal_type: u8, // 1=Volume, 2=Distribution, 3=Cardinality, 4=Burst, 5=Spectral, 6=ChangePoint, 7=RRCF, 8=MultiScale, 9=Behavioral, 10=Drift
    pub expected: f64,   // Contextual expected value (for UI)
    pub confidence: f64, // Detector confidence in this result (0.0 to 1.0)
    pub reason: String,  // Human-readable reason for detection
}

/// The Interface that all SOTA detectors must implement
pub trait Detector: Send + Sync {
    fn name(&self) -> &str;
    /// Update state and return anomaly score
    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult>;
    /// Get detector statistics for debugging
    fn get_stats(&self) -> String {
        String::new()
    }
}

// --- Enhanced Concrete Detectors with Adaptive Thresholds ---

/// 1. Volume Detector (RPS via Holt-Winters + Adaptive Threshold)
/// Tracks the rate of events and predicts trends/seasonality with adaptive sensitivity.
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
            adaptive_threshold: presets::volume_threshold(), // Uses 2-sigma adaptive threshold
            last_timestamp: 0,
            warmup_count: 0,
        }
    }
}

impl Detector for VolumeDetectorV2 {
    fn name(&self) -> &str {
        "Volume/RPS-V2"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        // Calculate Instantaneous RPS
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

        // Update Holt-Winters
        let (predicted, deviation) = self.hw.update(smoothed_rps);

        // During warmup, just learn the pattern
        if ctx.is_warmup || self.warmup_count < 100 {
            return None;
        }

        // Use adaptive threshold on the deviation
        let _threshold = self.adaptive_threshold.update(deviation.abs());
        let score = self.adaptive_threshold.anomaly_score(deviation.abs());

        // Calculate confidence based on prediction quality
        let prediction_error = deviation.abs() / predicted.max(1.0);
        let confidence = if prediction_error < 0.1 {
            0.9
        } else if prediction_error < 0.3 {
            0.7
        } else {
            0.5
        };

        if score > 0.0 {
            let reason = format!(
                "Volume {}: expected {:.1} RPS, observed {:.1} RPS (deviation: {:.1})",
                if deviation > 0.0 { "spike" } else { "drop" },
                predicted,
                smoothed_rps,
                deviation
            );

            Some(DetectionResult {
                score,
                weight: 1.0,
                signal_type: 1,
                expected: predicted,
                confidence,
                reason,
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

/// 2. Distribution Detector (Latency/Value via Fading Histogram + Adaptive)
/// Detects distribution shifts with adaptive probability thresholds.
pub struct DistributionDetectorV2 {
    hist: FadingHistogram,
    adaptive_threshold: AdaptiveThreshold,
}

impl DistributionDetectorV2 {
    pub fn new(bins: usize, min: f64, max: f64, decay: f64) -> Self {
        Self {
            hist: FadingHistogram::new(bins, min, max, decay),
            adaptive_threshold: presets::distribution_threshold(), // 3-sigma conservative
        }
    }
}

impl Detector for DistributionDetectorV2 {
    fn name(&self) -> &str {
        "Distribution/Value-V2"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let anomaly_likelihood = self.hist.update(ctx.value);

        // Update adaptive threshold on likelihood
        let _threshold = self.adaptive_threshold.update(anomaly_likelihood);
        let score = self.adaptive_threshold.anomaly_score(anomaly_likelihood);

        // Confidence based on histogram maturity (inverse probability stability)
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
            let reason = format!(
                "Distribution shift: value {:.2} has rarity score {:.1} (extremely rare)",
                ctx.value, anomaly_likelihood
            );

            Some(DetectionResult {
                score,
                weight: 0.8,
                signal_type: 2,
                expected: 0.0,
                confidence,
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (mean, std, thresh, count) = self.adaptive_threshold.get_stats();
        format!(
            "DistV2: μ={:.2}, σ={:.2}, thresh={:.2}, n={}",
            mean, std, thresh, count
        )
    }
}

/// 3. Cardinality Detector (HLL Velocity + Adaptive Threshold)
/// Detects sudden influx of NEW unique items with adaptive velocity tracking.
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
            adaptive_threshold: presets::cardinality_threshold(), // Percentile-based
            last_count: 0.0,
            last_velocity: 0.0,
        }
    }
}

impl Detector for CardinalityDetectorV2 {
    fn name(&self) -> &str {
        "Cardinality/Velocity-V2"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.hll.add_hash(ctx.unique_id_hash);

        // Check growth
        let current_count = self.hll.count();
        let delta = current_count - self.last_count;
        self.last_count = current_count;

        // Track velocity with adaptive threshold
        let velocity = if delta > 0.0 { delta } else { 0.0 };
        self.last_velocity = self.velocity_tracker.update(velocity);

        // Use adaptive threshold on velocity
        let _threshold = self.adaptive_threshold.update(velocity);
        let score = self.adaptive_threshold.anomaly_score(velocity);

        // High confidence for cardinality changes (usually clear signal)
        let confidence = if velocity > self.last_velocity * 10.0 {
            0.95
        } else {
            0.85
        };

        if score > 0.0 {
            let reason = format!(
                "New unique entities: {:.0} new (velocity: {:.1}/event, normal: {:.1})",
                delta, velocity, self.last_velocity
            );

            Some(DetectionResult {
                score,
                weight: 1.2, // Very high importance for security
                signal_type: 3,
                expected: self.last_velocity,
                confidence,
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (mean, std, thresh, count) = self.adaptive_threshold.get_stats();
        format!(
            "CardV2: unique={:.0}, velocity_μ={:.2}, thresh={:.2}",
            self.last_count, mean, thresh
        )
    }
}

/// 4. Burst Detector (Enhanced CUSUM with V-Mask + FIR)
/// Detects tight clustering with SOTA change point detection.
pub struct BurstDetectorV2 {
    cusum: EnhancedCUSUM,
    iat_ewma: EWMA,
    last_timestamp: u64,
}

impl BurstDetectorV2 {
    pub fn new() -> Self {
        // CUSUM: target=50ms IAT, slack=10ms, threshold=4
        Self {
            cusum: EnhancedCUSUM::with_options(50.0, 10.0, 4.0, 5, true, 0.5),
            iat_ewma: EWMA::new(20.0),
            last_timestamp: 0,
        }
    }
}

impl Detector for BurstDetectorV2 {
    fn name(&self) -> &str {
        "Burst/IAT-V2"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        let delta_ns = ctx.timestamp.saturating_sub(self.last_timestamp);
        let delta_ms = delta_ns as f64 / 1_000_000.0;
        self.last_timestamp = ctx.timestamp;

        // Update CUSUM with IAT deviation from expected (50ms)
        // Negative deviation means faster arrival (burst)
        let burst_indicator = 100.0 - delta_ms; // Higher = more burst-like
        let alarm = self.cusum.update(burst_indicator);

        if alarm {
            let severity = self.cusum.alarm_severity;
            let alarm_type = self.cusum.alarm_type;

            let reason = format!(
                "Burst detected: IAT {:.1}ms (type: {}, severity: {:.0}%)",
                delta_ms,
                if alarm_type > 0 {
                    "clustering"
                } else {
                    "dispersion"
                },
                severity * 100.0
            );

            Some(DetectionResult {
                score: severity,
                weight: 0.6,
                signal_type: 4,
                expected: 50.0, // Expected IAT in ms
                confidence: 0.75,
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (c_pos, c_neg, thresh, total) = self.cusum.get_stats();
        format!(
            "BurstV2: C+={:.1}, C-={:.1}, thresh={:.1}, alarms={}",
            c_pos, c_neg, thresh, total
        )
    }
}

/// 5. Spectral Residual Detector (SOTA from Microsoft Azure)
/// FFT-based anomaly detection with zero hyperparameters.
pub struct SpectralDetector {
    spectral: SpectralResidual,
    value_buffer: Vec<f64>,
    last_values: Vec<f64>,
}

impl SpectralDetector {
    pub fn new() -> Self {
        Self {
            spectral: SpectralResidual::new(24, 0.6), // 24-point window, high sensitivity
            value_buffer: Vec::with_capacity(24),
            last_values: Vec::with_capacity(5),
        }
    }
}

impl Detector for SpectralDetector {
    fn name(&self) -> &str {
        "Spectral/FFT"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        // Track recent values for context
        self.last_values.push(ctx.value);
        if self.last_values.len() > 5 {
            self.last_values.remove(0);
        }

        // Update spectral residual detector
        let (score, is_anomaly) = self.spectral.update(ctx.value);

        if is_anomaly && score > 0.3 {
            // Calculate trend direction from last values
            let trend = if self.last_values.len() >= 2 {
                let first = self.last_values.first().unwrap_or(&ctx.value);
                let last = self.last_values.last().unwrap_or(&ctx.value);
                if last > first {
                    "spike"
                } else {
                    "drop"
                }
            } else {
                "anomaly"
            };

            let reason = format!(
                "Spectral anomaly detected: {} in value {:.2} (FFT residual score: {:.2})",
                trend, ctx.value, score
            );

            Some(DetectionResult {
                score,
                weight: 1.0,
                signal_type: 5,
                expected: 0.0, // Spectral doesn't predict single value
                confidence: 0.85,
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (window_size, mean, std) = self.spectral.get_stats();
        format!(
            "Spectral: window={}, μ={:.2}, σ={:.2}",
            window_size, mean, std
        )
    }
}

/// 6. Change Point Detector (Enhanced CUSUM for trend changes)
/// Detects sustained trend changes using CUSUM with V-mask.
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

impl Detector for ChangePointDetector {
    fn name(&self) -> &str {
        "ChangePoint/Trend"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_value == 0.0 {
            self.last_value = ctx.value;
            return None;
        }

        // Calculate value change
        let change = ctx.value - self.last_value;
        self.last_value = ctx.value;

        // Smooth the changes
        let smoothed_change = self.trend_ewma.update(change);

        // Update CUSUM on smoothed changes
        let alarm = self.cusum.update(smoothed_change);

        if alarm {
            let severity = self.cusum.alarm_severity;
            let alarm_type = self.cusum.alarm_type;

            let direction = if alarm_type > 0 {
                "increasing"
            } else {
                "decreasing"
            };

            let reason = format!(
                "Trend change detected: sustained {} trend (change: {:.2}, severity: {:.0}%)",
                direction,
                smoothed_change,
                severity * 100.0
            );

            Some(DetectionResult {
                score: severity,
                weight: 0.9,
                signal_type: 6,
                expected: 0.0,
                confidence: 0.8,
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (c_pos, c_neg, thresh, total) = self.cusum.get_stats();
        format!(
            "ChangePoint: C+={:.1}, C-={:.1}, thresh={:.1}, alarms={}",
            c_pos, c_neg, thresh, total
        )
    }
}

/// 7. RRCF Detector (Robust Random Cut Forest for multivariate detection)
/// Isolation-based anomaly detection with tree-based scoring.
pub struct RRCFDetectorV2 {
    rrcf: RRCFDetector,
    warmup_count: usize,
}

impl RRCFDetectorV2 {
    pub fn new() -> Self {
        Self {
            rrcf: RRCFDetector::new_univariate(10), // 10 shingle size
            warmup_count: 0,
        }
    }
}

impl Detector for RRCFDetectorV2 {
    fn name(&self) -> &str {
        "RRCF/Multivariate"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        // Update RRCF and get anomaly score
        let (score, is_anomaly) = self.rrcf.update(ctx.value);
        self.warmup_count += 1;

        // Need warmup and anomaly detected
        if self.warmup_count > 20 && is_anomaly && score > 0.5 {
            let reason = format!(
                "RRCF anomaly detected: co-displacement score {:.2} (isolation depth: deep)",
                score
            );

            Some(DetectionResult {
                score,
                weight: 1.1,
                signal_type: 7,
                expected: 0.0,
                confidence: (score * 0.9).min(0.95),
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        format!("RRCF: shingle=10, warmup={}/20", self.warmup_count)
    }
}

/// 8. Multi-Scale Detector (Temporal analysis at multiple resolutions)
/// Detects anomalies across different time scales simultaneously.
pub struct MultiScaleDetectorV2 {
    multi_scale: MultiScaleDetector,
    last_timestamp: u64,
}

impl MultiScaleDetectorV2 {
    pub fn new() -> Self {
        Self {
            multi_scale: MultiScaleDetector::new(),
            last_timestamp: 0,
        }
    }
}

impl Detector for MultiScaleDetectorV2 {
    fn name(&self) -> &str {
        "MultiScale/Temporal"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.last_timestamp = ctx.timestamp;

        // Update multi-scale detector
        let result = self.multi_scale.update(ctx.value, ctx.timestamp);

        if result.is_anomaly && result.combined_score > 0.5 {
            let scales_triggered = result
                .active_scales
                .iter()
                .filter(|(_, s, _)| *s > 0.5)
                .count();

            let reason = format!(
                "Multi-scale anomaly: detected at {} resolution(s), score {:.2}",
                scales_triggered, result.combined_score
            );

            Some(DetectionResult {
                score: result.combined_score,
                weight: 1.0,
                signal_type: 8,
                expected: 0.0,
                confidence: 0.75 + (scales_triggered as f64 * 0.05).min(0.2),
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let stats = self.multi_scale.get_stats();
        let active_count = stats.iter().filter(|(_, _, score, _)| *score > 0.5).count();
        format!("MultiScale: scales=4, active={}", active_count)
    }
}

/// 9. Behavioral Fingerprint Detector (Per-entity behavioral profiling)
/// Tracks unique behavioral patterns for each entity ID.
pub struct BehavioralFingerprintDetectorV2 {
    behavioral: BehavioralFingerprintDetector,
    last_timestamp: u64,
    last_entity: u64,
}

impl BehavioralFingerprintDetectorV2 {
    pub fn new() -> Self {
        Self {
            behavioral: BehavioralFingerprintDetector::new(1000), // 1000 max profiles
            last_timestamp: 0,
            last_entity: 0,
        }
    }
}

impl Detector for BehavioralFingerprintDetectorV2 {
    fn name(&self) -> &str {
        "Behavioral/Fingerprint"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        // Use value as payload size for behavioral analysis
        let (score, is_anomaly, reason) = self.behavioral.process(
            ctx.unique_id_hash,
            ctx.timestamp,
            ctx.value.abs(),                     // Use absolute value as payload size
            ctx.unique_id_hash.wrapping_mul(31), // Derive service hash
        );

        self.last_timestamp = ctx.timestamp;
        self.last_entity = ctx.unique_id_hash;

        // Score represents deviation from entity's normal behavior
        if is_anomaly && score > 0.6 {
            Some(DetectionResult {
                score,
                weight: 1.2, // High weight for entity-specific anomalies
                signal_type: 9,
                expected: 0.0,
                confidence: (score * 0.85).min(0.95),
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (num_profiles, _, _) = self.behavioral.get_stats();
        format!("Behavioral: entities={}", num_profiles)
    }
}

/// 10. Drift Detector (Concept drift and distribution shift detection)
/// Monitors for gradual changes in data distribution over time.
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

impl Detector for DriftDetectorV2 {
    fn name(&self) -> &str {
        "Drift/Concept"
    }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.sample_count += 1;

        // Need sufficient history for drift detection
        if self.sample_count < 100 {
            return None;
        }

        // Update drift detector with current value
        let (drift_type, severity) = self.drift.update(ctx.value);

        if drift_type != DriftType::None {
            let drift_name = match drift_type {
                DriftType::Sudden => "sudden shift",
                DriftType::Gradual => "gradual drift",
                DriftType::Incremental => "incremental change",
                DriftType::Seasonal => "seasonal pattern",
                DriftType::None => "unknown",
            };

            let reason = format!(
                "Concept drift detected: {} (severity: {:.0}%)",
                drift_name,
                severity * 100.0
            );

            Some(DetectionResult {
                score: severity,
                weight: 0.9,
                signal_type: 10,
                expected: 0.0,
                confidence: 0.7 + (severity * 0.25),
                reason,
            })
        } else {
            None
        }
    }

    fn get_stats(&self) -> String {
        let (samples, drift_type, _, _history_len) = self.drift.get_stats();
        format!(
            "Drift: samples={}, detected={}",
            samples,
            if drift_type != DriftType::None {
                "yes"
            } else {
                "no"
            }
        )
    }
}

// --- Enhanced Ensemble Engine ---

pub struct AnomalyProfileV2 {
    detectors: Vec<Box<dyn Detector>>,
    event_count: u64,
    // Track detector performance for adaptive weighting
    detector_hits: Vec<u64>,
    // Ensemble confidence threshold
    confidence_threshold: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AnomalyResultV2 {
    pub is_anomaly: bool,
    pub severity: u8,
    pub anomaly_score: f64,
    pub signal_type: u8,
    pub expected: f64,
    pub actual: f64,
    pub confidence: f64,
}

/// Convert V2 result to legacy format for FFI compatibility
impl From<AnomalyResultV2> for super::AnomalyResult {
    fn from(v2: AnomalyResultV2) -> Self {
        super::AnomalyResult {
            is_anomaly: v2.is_anomaly,
            severity: v2.severity,
            anomaly_score: v2.anomaly_score,
            signal_type: v2.signal_type,
            expected: v2.expected,
            actual: v2.actual,
        }
    }
}

impl AnomalyProfileV2 {
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
        // Construct the enhanced SOTA pipeline with 10 detectors
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(VolumeDetectorV2::new(hw_alpha, hw_beta, hw_gamma, period)),
            Box::new(DistributionDetectorV2::new(
                hist_bins, min_val, max_val, hist_decay,
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

        let num_detectors = detectors.len();

        Self {
            detectors,
            event_count: 0,
            detector_hits: vec![0; num_detectors],
            confidence_threshold: 0.6,
        }
    }

    /// Create with default parameters (convenience constructor)
    pub fn default() -> Self {
        Self::new(0.3, 0.1, 0.1, 24, 50, 0.0, 10000.0, 0.999)
    }

    /// Zero-Allocation Hot Path
    pub fn process_with_hash(
        &mut self,
        timestamp: u64,
        unique_id_hash: u64,
        value: f64,
    ) -> AnomalyResultV2 {
        self.event_count += 1;

        let ctx = SignalContext {
            timestamp,
            unique_id_hash,
            value,
            is_warmup: self.event_count < 100, // Extended warmup for new detectors
        };

        // Enhanced Ensemble Voting with confidence weighting
        let mut weighted_score = 0.0;
        let mut total_confidence_weight = 0.0;
        let mut max_signal_type = 0;
        let mut primary_expected = 0.0;
        let mut detection_count = 0;
        let mut total_confidence = 0.0;

        for (idx, detector) in self.detectors.iter_mut().enumerate() {
            if let Some(res) = detector.update(&ctx) {
                // Weight by both detector weight and confidence
                let effective_weight = res.weight * res.confidence;
                weighted_score += res.score * effective_weight;
                total_confidence_weight += effective_weight;
                total_confidence += res.confidence;
                detection_count += 1;
                self.detector_hits[idx] += 1;

                // Track primary signal type (highest weighted score wins)
                if res.score * effective_weight > weighted_score - (res.score * effective_weight) {
                    max_signal_type = res.signal_type;
                    primary_expected = res.expected;
                }
            }
        }

        // Calculate final score with confidence normalization
        let final_score = if total_confidence_weight > 0.0 {
            (weighted_score / total_confidence_weight)
                * (detection_count as f64 / self.detectors.len() as f64).min(1.0)
        } else {
            0.0
        };

        // Average confidence across triggering detectors
        let avg_confidence = if detection_count > 0 {
            total_confidence / detection_count as f64
        } else {
            1.0 // No detections = confident it's normal
        };

        // Enhanced severity calculation with confidence gating
        let is_anomaly = final_score > 0.5 && avg_confidence >= self.confidence_threshold;
        let severity = if final_score > 0.85 {
            3
        }
        // Critical
        else if final_score > 0.7 {
            2
        }
        // High
        else if is_anomaly {
            1
        }
        // Low
        else {
            0
        }; // Normal

        AnomalyResultV2 {
            is_anomaly,
            severity,
            anomaly_score: final_score,
            signal_type: max_signal_type,
            expected: primary_expected,
            actual: value,
            confidence: avg_confidence,
        }
    }

    /// Get detector statistics for monitoring/debugging
    pub fn get_detector_stats(&self) -> Vec<(String, u64, String)> {
        self.detectors
            .iter()
            .enumerate()
            .map(|(idx, det)| {
                (
                    det.name().to_string(),
                    self.detector_hits[idx],
                    det.get_stats(),
                )
            })
            .collect()
    }

    /// Reset all detector state
    pub fn reset(&mut self) {
        self.event_count = 0;
        self.detector_hits = vec![0; self.detectors.len()];
        // Note: Individual detector state is preserved unless explicitly reset
    }
}

// --- Factory for creating appropriate profile version ---

pub enum AnomalyProfileType {
    Legacy, // Original 4 detectors
    V2,     // Enhanced 6 detectors with adaptive thresholds
}

/// Factory function to create appropriate profile
pub fn create_profile(
    profile_type: AnomalyProfileType,
    hw_alpha: f64,
    hw_beta: f64,
    hw_gamma: f64,
    period: usize,
    hist_bins: usize,
    min_val: f64,
    max_val: f64,
    hist_decay: f64,
) -> Box<dyn AnomalyProfileTrait> {
    match profile_type {
        AnomalyProfileType::Legacy => Box::new(super::AnomalyProfile::new(
            hw_alpha, hw_beta, hw_gamma, period, hist_bins, min_val, max_val, hist_decay,
        )),
        AnomalyProfileType::V2 => Box::new(AnomalyProfileV2::new(
            hw_alpha, hw_beta, hw_gamma, period, hist_bins, min_val, max_val, hist_decay,
        )),
    }
}

/// Trait for polymorphic profile usage
pub trait AnomalyProfileTrait: Send + Sync {
    fn process_with_hash(
        &mut self,
        timestamp: u64,
        unique_id_hash: u64,
        value: f64,
    ) -> super::AnomalyResult;
    fn get_stats(&self) -> String;
}

impl AnomalyProfileTrait for AnomalyProfileV2 {
    fn process_with_hash(
        &mut self,
        timestamp: u64,
        unique_id_hash: u64,
        value: f64,
    ) -> super::AnomalyResult {
        let v2_result = self.process_with_hash(timestamp, unique_id_hash, value);
        v2_result.into()
    }

    fn get_stats(&self) -> String {
        format!("V2 Profile: {} events processed", self.event_count)
    }
}
