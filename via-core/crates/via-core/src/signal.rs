//! Rich Signal Output for Tier-2 Consumption
//!
//! This module defines the comprehensive anomaly signal that Tier-1 emits.
//! Unlike the minimal AnomalyResult, this provides full detector breakdown,
//! SHAP-like attribution, and contextual information for Tier-2 reasoning.

use serde::{Deserialize, Serialize};

/// Number of detectors in the ensemble (compile-time constant)
pub const NUM_DETECTORS: usize = 10;

/// Detector identifiers for attribution
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectorId {
    Volume = 0,
    Distribution = 1,
    Cardinality = 2,
    Burst = 3,
    Spectral = 4,
    ChangePoint = 5,
    RRCF = 6,
    MultiScale = 7,
    Behavioral = 8,
    Drift = 9,
}

impl DetectorId {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Volume),
            1 => Some(Self::Distribution),
            2 => Some(Self::Cardinality),
            3 => Some(Self::Burst),
            4 => Some(Self::Spectral),
            5 => Some(Self::ChangePoint),
            6 => Some(Self::RRCF),
            7 => Some(Self::MultiScale),
            8 => Some(Self::Behavioral),
            9 => Some(Self::Drift),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Volume => "Volume/RPS",
            Self::Distribution => "Distribution/Value",
            Self::Cardinality => "Cardinality/Velocity",
            Self::Burst => "Burst/IAT",
            Self::Spectral => "Spectral/FFT",
            Self::ChangePoint => "ChangePoint/Trend",
            Self::RRCF => "RRCF/Isolation",
            Self::MultiScale => "MultiScale/Temporal",
            Self::Behavioral => "Behavioral/Fingerprint",
            Self::Drift => "Drift/Concept",
        }
    }
}

/// Severity levels for anomalies
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Severity {
    #[default]
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl Severity {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.9 {
            Self::Critical
        } else if score >= 0.75 {
            Self::High
        } else if score >= 0.6 {
            Self::Medium
        } else if score >= 0.4 {
            Self::Low
        } else {
            Self::None
        }
    }
}

/// Individual detector score (fixed size for zero-allocation)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DetectorScore {
    /// Raw anomaly score from detector (0.0 - 1.0)
    pub score: f32,
    /// Detector's self-assessed confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Whether this detector triggered (exceeded its threshold)
    pub fired: bool,
    /// Expected value (for context)
    pub expected: f32,
    /// Observed value
    pub observed: f32,
}

impl DetectorScore {
    pub fn new(score: f64, confidence: f64, fired: bool, expected: f64, observed: f64) -> Self {
        Self {
            score: score as f32,
            confidence: confidence as f32,
            fired,
            expected: expected as f32,
            observed: observed as f32,
        }
    }

    /// Weight-adjusted contribution to ensemble
    pub fn weighted_contribution(&self, weight: f64) -> f64 {
        self.score as f64 * self.confidence as f64 * weight
    }
}

/// Baseline behavioral summary for context
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BaselineSummary {
    /// Average value seen for this entity
    pub avg_value: f32,
    /// Standard deviation of values
    pub std_value: f32,
    /// Average events per second
    pub avg_frequency: f32,
    /// Total events processed for this profile
    pub profile_age: u32,
    /// Whether profile is in warmup period
    pub is_warmup: bool,
}

/// Attribution: Which detectors contributed most to the decision
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Attribution {
    /// Primary contributing detector
    pub primary_detector: u8,
    /// Secondary contributing detector
    pub secondary_detector: u8,
    /// Primary detector's contribution (0.0 - 1.0)
    pub primary_contribution: f32,
    /// Secondary detector's contribution (0.0 - 1.0)
    pub secondary_contribution: f32,
    /// Number of detectors that fired
    pub detectors_fired: u8,
}

impl Attribution {
    /// Compute attribution from detector scores and weights
    pub fn compute(scores: &[DetectorScore; NUM_DETECTORS], weights: &[f64; NUM_DETECTORS]) -> Self {
        let mut contributions: [(usize, f64); NUM_DETECTORS] = [(0, 0.0); NUM_DETECTORS];
        let mut detectors_fired = 0u8;

        for (i, (score, weight)) in scores.iter().zip(weights.iter()).enumerate() {
            if score.fired {
                detectors_fired += 1;
            }
            contributions[i] = (i, score.weighted_contribution(*weight));
        }

        // Sort by contribution (descending)
        contributions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total: f64 = contributions.iter().map(|(_, c)| c).sum();
        let normalize = if total > 0.0 { total } else { 1.0 };

        Self {
            primary_detector: contributions[0].0 as u8,
            secondary_detector: contributions[1].0 as u8,
            primary_contribution: (contributions[0].1 / normalize) as f32,
            secondary_contribution: (contributions[1].1 / normalize) as f32,
            detectors_fired,
        }
    }
}

/// Full anomaly signal for Tier-2 consumption
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalySignal {
    // === Identity ===
    /// Entity hash (xxhash of user/session ID)
    pub entity_hash: u64,
    /// Event timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// Sequence number for this entity
    pub sequence: u64,

    // === Primary Decision ===
    /// Whether this is classified as an anomaly
    pub is_anomaly: bool,
    /// Severity level
    pub severity: Severity,
    /// Combined ensemble score (0.0 - 1.0)
    pub ensemble_score: f64,
    /// Overall confidence in the decision
    pub confidence: f64,

    // === Full Detector Breakdown ===
    /// Individual scores from all 10 detectors
    pub detector_scores: [DetectorScore; NUM_DETECTORS],
    /// Current ensemble weights for each detector
    pub detector_weights: [f32; NUM_DETECTORS],

    // === Attribution ===
    /// Which detectors contributed most
    pub attribution: Attribution,

    // === Context ===
    /// Baseline behavior for this entity
    pub baseline: BaselineSummary,
    /// Raw value that was processed
    pub raw_value: f64,
}

impl Default for AnomalySignal {
    fn default() -> Self {
        Self {
            entity_hash: 0,
            timestamp: 0,
            sequence: 0,
            is_anomaly: false,
            severity: Severity::None,
            ensemble_score: 0.0,
            confidence: 1.0,
            detector_scores: [DetectorScore::default(); NUM_DETECTORS],
            detector_weights: [0.1; NUM_DETECTORS], // Equal weights initially
            attribution: Attribution::default(),
            baseline: BaselineSummary::default(),
            raw_value: 0.0,
        }
    }
}

impl AnomalySignal {
    /// Create a new signal builder
    pub fn builder(entity_hash: u64, timestamp: u64) -> AnomalySignalBuilder {
        AnomalySignalBuilder::new(entity_hash, timestamp)
    }

    /// Get detector name for primary attribution
    pub fn primary_detector_name(&self) -> &'static str {
        DetectorId::from_u8(self.attribution.primary_detector)
            .map(|d| d.name())
            .unwrap_or("Unknown")
    }

    /// Get detector name for secondary attribution
    pub fn secondary_detector_name(&self) -> &'static str {
        DetectorId::from_u8(self.attribution.secondary_detector)
            .map(|d| d.name())
            .unwrap_or("Unknown")
    }

    /// Check if specific detector fired
    pub fn detector_fired(&self, detector: DetectorId) -> bool {
        self.detector_scores[detector as usize].fired
    }

    /// Get score for specific detector
    pub fn detector_score(&self, detector: DetectorId) -> f32 {
        self.detector_scores[detector as usize].score
    }

    /// Generate a compact reason string
    pub fn reason(&self) -> String {
        if !self.is_anomaly {
            return String::from("Normal behavior");
        }

        let primary = self.primary_detector_name();
        let secondary = self.secondary_detector_name();
        let fired = self.attribution.detectors_fired;

        format!(
            "{} anomaly (score: {:.2}, confidence: {:.0}%) - Primary: {} ({:.0}%), Secondary: {} ({:.0}%), {} detectors triggered",
            match self.severity {
                Severity::Critical => "CRITICAL",
                Severity::High => "HIGH",
                Severity::Medium => "MEDIUM",
                Severity::Low => "LOW",
                Severity::None => "NONE",
            },
            self.ensemble_score,
            self.confidence * 100.0,
            primary,
            self.attribution.primary_contribution * 100.0,
            secondary,
            self.attribution.secondary_contribution * 100.0,
            fired
        )
    }
}

/// Builder for constructing AnomalySignal
pub struct AnomalySignalBuilder {
    signal: AnomalySignal,
}

impl AnomalySignalBuilder {
    pub fn new(entity_hash: u64, timestamp: u64) -> Self {
        Self {
            signal: AnomalySignal {
                entity_hash,
                timestamp,
                ..Default::default()
            },
        }
    }

    pub fn sequence(mut self, seq: u64) -> Self {
        self.signal.sequence = seq;
        self
    }

    pub fn raw_value(mut self, value: f64) -> Self {
        self.signal.raw_value = value;
        self
    }

    pub fn detector_score(mut self, detector: DetectorId, score: DetectorScore) -> Self {
        self.signal.detector_scores[detector as usize] = score;
        self
    }

    pub fn detector_weights(mut self, weights: [f64; NUM_DETECTORS]) -> Self {
        for (i, w) in weights.iter().enumerate() {
            self.signal.detector_weights[i] = *w as f32;
        }
        self
    }

    pub fn baseline(mut self, baseline: BaselineSummary) -> Self {
        self.signal.baseline = baseline;
        self
    }

    pub fn finalize(mut self, ensemble_score: f64, confidence: f64) -> AnomalySignal {
        self.signal.ensemble_score = ensemble_score;
        self.signal.confidence = confidence;
        self.signal.severity = Severity::from_score(ensemble_score);
        self.signal.is_anomaly = ensemble_score >= 0.4 && confidence >= 0.5;

        // Compute attribution
        let weights: [f64; NUM_DETECTORS] = {
            let mut arr = [0.0; NUM_DETECTORS];
            for (i, w) in self.signal.detector_weights.iter().enumerate() {
                arr[i] = *w as f64;
            }
            arr
        };
        self.signal.attribution = Attribution::compute(&self.signal.detector_scores, &weights);

        self.signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_from_score() {
        assert_eq!(Severity::from_score(0.95), Severity::Critical);
        assert_eq!(Severity::from_score(0.8), Severity::High);
        assert_eq!(Severity::from_score(0.65), Severity::Medium);
        assert_eq!(Severity::from_score(0.45), Severity::Low);
        assert_eq!(Severity::from_score(0.2), Severity::None);
    }

    #[test]
    fn test_signal_builder() {
        let signal = AnomalySignal::builder(12345, 1000000)
            .sequence(1)
            .raw_value(150.0)
            .detector_score(
                DetectorId::Volume,
                DetectorScore::new(0.8, 0.9, true, 100.0, 150.0),
            )
            .detector_score(
                DetectorId::Distribution,
                DetectorScore::new(0.6, 0.85, true, 50.0, 150.0),
            )
            .finalize(0.75, 0.88);

        assert!(signal.is_anomaly);
        assert_eq!(signal.severity, Severity::High);
        assert!(signal.detector_fired(DetectorId::Volume));
        assert!(signal.detector_fired(DetectorId::Distribution));
        assert!(!signal.detector_fired(DetectorId::Cardinality));
    }

    #[test]
    fn test_attribution() {
        let mut scores = [DetectorScore::default(); NUM_DETECTORS];
        scores[0] = DetectorScore::new(0.9, 0.95, true, 0.0, 0.0); // Volume
        scores[1] = DetectorScore::new(0.7, 0.80, true, 0.0, 0.0); // Distribution
        scores[2] = DetectorScore::new(0.3, 0.70, false, 0.0, 0.0); // Cardinality

        let weights = [0.15, 0.12, 0.10, 0.08, 0.12, 0.10, 0.11, 0.08, 0.08, 0.06];

        let attr = Attribution::compute(&scores, &weights);

        assert_eq!(attr.primary_detector, 0); // Volume should be primary
        assert_eq!(attr.secondary_detector, 1); // Distribution secondary
        assert_eq!(attr.detectors_fired, 2);
    }
}
