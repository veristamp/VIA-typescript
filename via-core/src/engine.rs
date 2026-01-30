use crate::algo::{
    ewma::EWMA, 
    hll::HyperLogLog, 
    holtwinters::HoltWinters,
    histogram::FadingHistogram
};

// --- Core Abstractions ---

/// Context passed to every detector for every event
pub struct SignalContext<'a> {
    pub timestamp: u64,
    pub unique_id: &'a str,
    pub value: f64, // e.g., Latency or Payload Size
    pub is_warmup: bool,
}

/// Result returned by a single detector
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub score: f64,         // 0.0 (Normal) to 1.0 (Critical)
    pub weight: f64,        // How much this detector matters (0.0 to 1.0)
    pub signal_type: u8,    // 1=Volume, 2=Latency, 3=Cardinality, 4=Burst
    pub expected: f64,      // Contextual expected value (for UI)
}

/// The Interface that all SOTA detectors must implement
pub trait Detector: Send + Sync {
    fn name(&self) -> &str;
    /// Update state and return anomaly score
    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult>;
}

// --- Concrete Detectors ---

/// 1. Volume Detector (RPS via Holt-Winters)
/// Tracks the rate of events and predicts trends/seasonality.
pub struct VolumeDetector {
    hw: HoltWinters,
    rate_estimator: EWMA, // Smoothes the IAT to get stable RPS
    last_timestamp: u64,
}

impl VolumeDetector {
    pub fn new(alpha: f64, beta: f64, gamma: f64, period: usize) -> Self {
        Self {
            hw: HoltWinters::new(alpha, beta, gamma, period),
            rate_estimator: EWMA::new(50.0), // 50 events half-life for rate smoothing
            last_timestamp: 0,
        }
    }
}

impl Detector for VolumeDetector {
    fn name(&self) -> &str { "Volume/RPS" }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        // Calculate Instantaneous RPS (Requests Per Second)
        // 1e9 ns = 1 sec
        let delta_ns = ctx.timestamp.saturating_sub(self.last_timestamp).max(1);
        let delta_sec = delta_ns as f64 / 1_000_000_000.0;
        
        // Instant rate = 1 event / delta_sec
        // We smooth this because raw IAT is extremely noisy
        let instant_rps = if delta_sec > 0.0 { 1.0 / delta_sec } else { 0.0 };
        let smoothed_rps = self.rate_estimator.update(instant_rps);
        
        self.last_timestamp = ctx.timestamp;

        if ctx.is_warmup {
            self.hw.update(smoothed_rps);
            return None;
        }

        let (predicted, deviation) = self.hw.update(smoothed_rps);
        
        // Z-Score-like normalization
        // If deviation > 3x expected noise, we start flagging
        let threshold = predicted.max(10.0) * 0.2; // 20% tolerance or min 10 RPS
        let score = (deviation.abs() - threshold).max(0.0) / predicted.max(1.0);
        
        // Sigmoid squash to 0.0-1.0
        let final_score = (score / 4.0).clamp(0.0, 1.0);

        if final_score > 0.1 {
            Some(DetectionResult {
                score: final_score,
                weight: 1.0, // High importance
                signal_type: 1,
                expected: predicted,
            })
        } else {
            None
        }
    }
}

/// 2. Distribution Detector (Latency/Value via Fading Histogram)
/// Detects when the "Shape" of the data changes (e.g., Latency P99 spike).
pub struct DistributionDetector {
    hist: FadingHistogram,
}

impl DistributionDetector {
    pub fn new(bins: usize, min: f64, max: f64, decay: f64) -> Self {
        Self {
            hist: FadingHistogram::new(bins, min, max, decay),
        }
    }
}

impl Detector for DistributionDetector {
    fn name(&self) -> &str { "Distribution/Value" }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        let anomaly_likelihood = self.hist.update(ctx.value);
        
        // likelihood is "Inverse Probability". High = Anomaly.
        // > 10.0 means < 10% probability
        // > 50.0 means < 2% probability
        
        let score = if anomaly_likelihood > 20.0 {
            ((anomaly_likelihood - 20.0) / 80.0).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if score > 0.0 {
            Some(DetectionResult {
                score,
                weight: 0.8, // Medium-High importance
                signal_type: 2,
                expected: 0.0, // Hard to predict single expected value for distribution
            })
        } else {
            None
        }
    }
}

/// 3. Cardinality Detector (HLL Velocity)
/// Detects sudden influx of NEW unique items (e.g., Credential Stuffing).
pub struct CardinalityDetector {
    hll: HyperLogLog,
    last_count: f64,
    velocity_ewma: EWMA,
}

impl CardinalityDetector {
    pub fn new() -> Self {
        Self {
            hll: HyperLogLog::new(12),
            last_count: 0.0,
            velocity_ewma: EWMA::new(100.0),
        }
    }
}

impl Detector for CardinalityDetector {
    fn name(&self) -> &str { "Cardinality/Velocity" }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        self.hll.add(ctx.unique_id);
        
        // Check growth
        let current_count = self.hll.count();
        let delta = current_count - self.last_count;
        self.last_count = current_count;
        
        // Track the "Velocity" of new users
        let avg_velocity = self.velocity_ewma.update(delta);
        
        // If we suddenly see 5x the normal rate of new users -> Anomaly
        // This is classic Credential Stuffing detection
        let ratio = if avg_velocity > 0.1 { delta / avg_velocity } else { 0.0 };
        
        let score = if ratio > 3.0 {
            ((ratio - 3.0) / 7.0).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if score > 0.0 {
             Some(DetectionResult {
                score,
                weight: 1.2, // Very High importance (Security threat)
                signal_type: 3,
                expected: avg_velocity,
            })
        } else {
            None
        }
    }
}

/// 4. Burst Detector (Micro-bursts via IAT)
/// Detects tight clustering of events (e.g., DoS, Scripted Attacks).
pub struct BurstDetector {
    iat_ewma: EWMA,
    last_timestamp: u64,
}

impl BurstDetector {
    pub fn new() -> Self {
        Self {
            iat_ewma: EWMA::new(20.0), // Very fast reaction
            last_timestamp: 0,
        }
    }
}

impl Detector for BurstDetector {
    fn name(&self) -> &str { "Burst/IAT" }

    fn update(&mut self, ctx: &SignalContext) -> Option<DetectionResult> {
        if self.last_timestamp == 0 {
            self.last_timestamp = ctx.timestamp;
            return None;
        }

        let delta_ns = ctx.timestamp.saturating_sub(self.last_timestamp);
        let delta_ms = delta_ns as f64 / 1_000_000.0;
        self.last_timestamp = ctx.timestamp;

        let avg_iat = self.iat_ewma.update(delta_ms);

        // If current event arrived 10x faster than average -> Burst
        // Only valid if average is somewhat paced (> 10ms)
        if avg_iat > 10.0 && delta_ms < (avg_iat * 0.1) {
             Some(DetectionResult {
                score: 0.5, // Bursts are often just noise, so lower confidence
                weight: 0.5,
                signal_type: 4,
                expected: avg_iat,
            })
        } else {
            None
        }
    }
}

// --- The Engine (Ensemble) ---

pub struct AnomalyProfile {
    detectors: Vec<Box<dyn Detector>>,
    event_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub severity: u8,
    pub anomaly_score: f64,
    pub signal_type: u8,
    pub expected: f64,
    pub actual: f64,
}

impl AnomalyProfile {
    pub fn new(
        hw_alpha: f64, hw_beta: f64, hw_gamma: f64, period: usize,
        hist_bins: usize, min_val: f64, max_val: f64, hist_decay: f64
    ) -> Self {
        // Construct the SOTA pipeline
        let detectors: Vec<Box<dyn Detector>> = vec![
            Box::new(VolumeDetector::new(hw_alpha, hw_beta, hw_gamma, period)),
            Box::new(DistributionDetector::new(hist_bins, min_val, max_val, hist_decay)),
            Box::new(CardinalityDetector::new()),
            Box::new(BurstDetector::new()),
        ];

        Self {
            detectors,
            event_count: 0,
        }
    }

    pub fn process(&mut self, timestamp: u64, unique_id: &str, value: f64) -> AnomalyResult {
        self.event_count += 1;
        
        let ctx = SignalContext {
            timestamp,
            unique_id,
            value,
            is_warmup: self.event_count < 50, // 50 event warmup
        };

        // Ensemble Voting
        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        let mut max_signal_type = 0;
        let mut primary_expected = 0.0;

        for detector in &mut self.detectors {
            if let Some(res) = detector.update(&ctx) {
                total_score += res.score * res.weight;
                total_weight += res.weight;
                
                // Track the "Primary" reason (highest score wins logic for UI)
                if res.score * res.weight > total_score - (res.score * res.weight) {
                     max_signal_type = res.signal_type;
                     primary_expected = res.expected;
                }
            }
        }

        // Normalize Score
        let final_score = if total_weight > 0.0 {
            total_score / 2.0 // Soft normalization, allowing accumulation
        } else {
            0.0
        };

        // Thresholds
        let is_anomaly = final_score > 0.5;
        let severity = if final_score > 0.8 { 3 } // High
                       else if final_score > 0.6 { 2 } // Med
                       else if is_anomaly { 1 } // Low
                       else { 0 };

        AnomalyResult {
            is_anomaly,
            severity,
            anomaly_score: final_score,
            signal_type: max_signal_type,
            expected: primary_expected,
            actual: value,
        }
    }
}
