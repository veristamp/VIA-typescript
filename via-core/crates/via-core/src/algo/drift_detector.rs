//! Concept Drift Detector
//!
//! Detects when data distribution changes over time, triggering model
//! retraining or adaptation. Uses statistical tests to compare recent
//! data against a reference window.
//!
//! Methods implemented:
//! - ADWIN (Adaptive Windowing) for incremental drift detection
//! - Kolmogorov-Smirnov test for distribution comparison
//! - KL-divergence for information-theoretic drift
//! - Page-Hinkley test for gradual drift

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Types of drift that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftType {
    /// No drift detected
    None,
    /// Sudden abrupt change
    Sudden,
    /// Gradual shift over time
    Gradual,
    /// Incremental drift (step-by-step)
    Incremental,
    /// Periodic/seasonal pattern change
    Seasonal,
}

/// ADWIN (Adaptive Windowing) algorithm
/// Automatically adjusts window size to maintain statistical guarantees
#[derive(Serialize, Deserialize, Clone)]
pub struct ADWIN {
    /// Reference window (older data)
    reference_window: VecDeque<f64>,
    /// Current window (recent data)
    current_window: VecDeque<f64>,
    /// Maximum window size
    max_window_size: usize,
    /// Minimum window size before testing
    min_window_size: usize,
    /// Confidence level (delta)
    delta: f64,
    /// Current drift status
    drift_detected: bool,
    /// Sum of reference window
    ref_sum: f64,
    /// Sum of current window
    curr_sum: f64,
    /// Sum of squares for reference
    ref_sum_sq: f64,
    /// Sum of squares for current
    curr_sum_sq: f64,
}

impl ADWIN {
    pub fn new(delta: f64, max_window_size: usize) -> Self {
        Self {
            reference_window: VecDeque::with_capacity(max_window_size),
            current_window: VecDeque::with_capacity(max_window_size),
            max_window_size: max_window_size.max(100),
            min_window_size: 30,
            delta: delta.clamp(0.001, 0.1),
            drift_detected: false,
            ref_sum: 0.0,
            curr_sum: 0.0,
            ref_sum_sq: 0.0,
            curr_sum_sq: 0.0,
        }
    }

    /// Update with new value and check for drift
    pub fn update(&mut self, value: f64) -> (DriftType, f64) {
        self.drift_detected = false;

        // Add to current window
        self.current_window.push_back(value);
        self.curr_sum += value;
        self.curr_sum_sq += value * value;

        // Maintain window size
        if self.current_window.len() > self.max_window_size / 2 {
            // Move oldest from current to reference
            let old = self.current_window.pop_front().unwrap();
            self.curr_sum -= old;
            self.curr_sum_sq -= old * old;

            self.reference_window.push_back(old);
            self.ref_sum += old;
            self.ref_sum_sq += old * old;

            // Trim reference if too large
            if self.reference_window.len() > self.max_window_size / 2 {
                let removed = self.reference_window.pop_front().unwrap();
                self.ref_sum -= removed;
                self.ref_sum_sq -= removed * removed;
            }
        }

        // Check for drift if windows are large enough
        if self.reference_window.len() >= self.min_window_size
            && self.current_window.len() >= self.min_window_size
        {
            let n_ref = self.reference_window.len() as f64;
            let n_curr = self.current_window.len() as f64;

            let mean_ref = self.ref_sum / n_ref;
            let mean_curr = self.curr_sum / n_curr;

            // Variance calculation (using online algorithm)
            let var_ref = (self.ref_sum_sq / n_ref) - (mean_ref * mean_ref);
            let var_curr = (self.curr_sum_sq / n_curr) - (mean_curr * mean_curr);

            let variance = ((n_ref * var_ref + n_curr * var_curr) / (n_ref + n_curr)).max(0.0001);

            // ADWIN test statistic
            let m = (1.0 / n_ref + 1.0 / n_curr).sqrt();
            let epsilon = (2.0 * variance * m * (2.0 / self.delta).ln()).sqrt()
                + 2.0 / 3.0 * m * (2.0 / self.delta).ln();

            let diff = (mean_ref - mean_curr).abs();

            if diff > epsilon {
                self.drift_detected = true;
                // Reset windows on drift
                self.reference_window.clear();
                self.current_window.clear();
                self.ref_sum = 0.0;
                self.curr_sum = 0.0;
                self.ref_sum_sq = 0.0;
                self.curr_sum_sq = 0.0;
                return (DriftType::Sudden, diff);
            }

            return (DriftType::None, diff / epsilon);
        }

        (DriftType::None, 0.0)
    }

    pub fn drift_detected(&self) -> bool {
        self.drift_detected
    }

    pub fn get_stats(&self) -> (usize, usize, bool) {
        (
            self.reference_window.len(),
            self.current_window.len(),
            self.drift_detected,
        )
    }
}

/// Page-Hinkley test for gradual drift detection
#[derive(Serialize, Deserialize, Clone)]
pub struct PageHinkley {
    /// Cumulative sum of deviations
    cum_sum: f64,
    /// Minimum cumulative sum seen
    min_cum_sum: f64,
    /// Mean of observed values
    mean: f64,
    /// Number of observations
    count: u64,
    /// Detection threshold
    threshold: f64,
    /// Sensitivity parameter (lambda)
    lambda: f64,
    /// Alpha for mean adaptation
    alpha: f64,
    /// Drift detected flag
    drift_detected: bool,
    /// Test statistic
    test_statistic: f64,
}

impl PageHinkley {
    pub fn new(threshold: f64, lambda: f64, alpha: f64) -> Self {
        Self {
            cum_sum: 0.0,
            min_cum_sum: 0.0,
            mean: 0.0,
            count: 0,
            threshold: threshold.max(10.0),
            lambda: lambda.max(0.0),
            alpha: alpha.clamp(0.001, 0.1),
            drift_detected: false,
            test_statistic: 0.0,
        }
    }

    /// Update with new value
    pub fn update(&mut self, value: f64) -> (DriftType, f64) {
        self.count += 1;
        self.drift_detected = false;

        // Update mean estimate
        if self.count == 1 {
            self.mean = value;
        } else {
            self.mean = self.alpha * value + (1.0 - self.alpha) * self.mean;
        }

        // Update cumulative sum of deviations
        self.cum_sum += value - self.mean - self.lambda;

        // Track minimum
        if self.cum_sum < self.min_cum_sum {
            self.min_cum_sum = self.cum_sum;
        }

        // Test statistic
        self.test_statistic = self.cum_sum - self.min_cum_sum;

        // Check threshold
        if self.test_statistic > self.threshold {
            self.drift_detected = true;
            // Reset
            self.cum_sum = 0.0;
            self.min_cum_sum = 0.0;
            self.mean = value;
            return (DriftType::Gradual, self.test_statistic / self.threshold);
        }

        (DriftType::None, self.test_statistic / self.threshold)
    }

    pub fn reset(&mut self) {
        self.cum_sum = 0.0;
        self.min_cum_sum = 0.0;
        self.mean = 0.0;
        self.count = 0;
        self.drift_detected = false;
        self.test_statistic = 0.0;
    }

    pub fn drift_detected(&self) -> bool {
        self.drift_detected
    }
}

/// KL-Divergence based drift detector
/// Compares distributions using information-theoretic measure
#[derive(Serialize, Deserialize, Clone)]
pub struct KLDivergenceDetector {
    /// Reference histogram (baseline distribution)
    reference_hist: Vec<u64>,
    /// Current histogram (recent distribution)
    current_hist: Vec<u64>,
    /// Number of bins
    num_bins: usize,
    /// Min and max for binning
    min_val: f64,
    max_val: f64,
    /// Threshold for detection
    threshold: f64,
    /// Sample count for current window
    current_count: u64,
    /// Target count before testing
    target_count: u64,
    /// Epsilon for numerical stability
    epsilon: f64,
}

impl KLDivergenceDetector {
    pub fn new(num_bins: usize, min_val: f64, max_val: f64, threshold: f64) -> Self {
        let bins = num_bins.max(10).min(1000);
        Self {
            reference_hist: vec![0; bins],
            current_hist: vec![0; bins],
            num_bins: bins,
            min_val,
            max_val,
            threshold: threshold.max(0.1),
            current_count: 0,
            target_count: 1000,
            epsilon: 1e-10,
        }
    }

    /// Update with new value
    pub fn update(&mut self, value: f64) -> (DriftType, f64) {
        // Bin the value
        let bin = self.value_to_bin(value);
        self.current_hist[bin] += 1;
        self.current_count += 1;

        // Check if ready to compare
        if self.current_count >= self.target_count {
            let kl_div = self.compute_kl_divergence();

            if kl_div > self.threshold {
                // Drift detected - reset current, make it reference
                self.reference_hist = self.current_hist.clone();
                self.current_hist = vec![0; self.num_bins];
                self.current_count = 0;
                return (DriftType::Incremental, kl_div);
            }

            // No drift - merge into reference gradually
            if self.current_count >= self.target_count * 2 {
                for i in 0..self.num_bins {
                    self.reference_hist[i] =
                        (self.reference_hist[i] / 2) + (self.current_hist[i] / 2);
                }
                self.current_hist = vec![0; self.num_bins];
                self.current_count = 0;
            }

            return (DriftType::None, kl_div / self.threshold);
        }

        (DriftType::None, 0.0)
    }

    /// Convert value to bin index
    fn value_to_bin(&self, value: f64) -> usize {
        let normalized = (value - self.min_val) / (self.max_val - self.min_val);
        let bin = (normalized * self.num_bins as f64) as usize;
        bin.min(self.num_bins - 1)
    }

    /// Compute KL divergence from reference to current
    fn compute_kl_divergence(&self) -> f64 {
        let ref_sum: u64 = self.reference_hist.iter().sum();
        let curr_sum: u64 = self.current_hist.iter().sum();

        if ref_sum == 0 || curr_sum == 0 {
            return 0.0;
        }

        let mut kl_div = 0.0;

        for i in 0..self.num_bins {
            let p = self.reference_hist[i] as f64 / ref_sum as f64;
            let q = self.current_hist[i] as f64 / curr_sum as f64;

            if p > self.epsilon && q > self.epsilon {
                kl_div += p * (p / q).ln();
            }
        }

        kl_div
    }
}

/// Ensemble drift detector combining multiple methods
#[derive(Serialize, Deserialize, Clone)]
pub struct EnsembleDriftDetector {
    adwin: ADWIN,
    page_hinkley: PageHinkley,
    kl_div: KLDivergenceDetector,
    /// Detection history
    drift_history: VecDeque<(u64, DriftType, f64)>,
    /// Current drift status
    current_drift: DriftType,
    /// Combined drift score
    drift_score: f64,
    /// Sample counter
    sample_count: u64,
}

impl EnsembleDriftDetector {
    pub fn new() -> Self {
        Self {
            adwin: ADWIN::new(0.002, 1000),
            page_hinkley: PageHinkley::new(50.0, 0.1, 0.01),
            kl_div: KLDivergenceDetector::new(50, 0.0, 1000.0, 0.5),
            drift_history: VecDeque::with_capacity(100),
            current_drift: DriftType::None,
            drift_score: 0.0,
            sample_count: 0,
        }
    }

    /// Update all detectors and return consensus
    pub fn update(&mut self, value: f64) -> (DriftType, f64) {
        self.sample_count += 1;

        // Update all detectors
        let (adwin_drift, adwin_score) = self.adwin.update(value);
        let (ph_drift, ph_score) = self.page_hinkley.update(value);
        let (kl_drift, kl_score) = self.kl_div.update(value);

        // Combine results (voting)
        let drift_detected = adwin_drift != DriftType::None
            || ph_drift != DriftType::None
            || kl_drift != DriftType::None;

        // Determine drift type (priority: Sudden > Gradual > Incremental)
        let drift_type = if adwin_drift == DriftType::Sudden {
            DriftType::Sudden
        } else if ph_drift == DriftType::Gradual {
            DriftType::Gradual
        } else if kl_drift == DriftType::Incremental {
            DriftType::Incremental
        } else {
            DriftType::None
        };

        // Combined score
        self.drift_score = (adwin_score + ph_score + kl_score) / 3.0;

        if drift_detected {
            self.current_drift = drift_type;
            self.drift_history
                .push_back((self.sample_count, drift_type, self.drift_score));
            if self.drift_history.len() > 100 {
                self.drift_history.pop_front();
            }
        } else {
            self.current_drift = DriftType::None;
        }

        (self.current_drift, self.drift_score)
    }

    /// Check if any drift detected
    pub fn drift_detected(&self) -> bool {
        self.current_drift != DriftType::None
    }

    /// Get current drift type
    pub fn drift_type(&self) -> DriftType {
        self.current_drift
    }

    /// Get drift history
    pub fn get_history(&self) -> Vec<(u64, DriftType, f64)> {
        self.drift_history.iter().cloned().collect()
    }

    /// Reset all detectors
    pub fn reset(&mut self) {
        self.adwin = ADWIN::new(0.002, 1000);
        self.page_hinkley = PageHinkley::new(50.0, 0.1, 0.01);
        self.kl_div = KLDivergenceDetector::new(50, 0.0, 1000.0, 0.5);
        self.drift_history.clear();
        self.current_drift = DriftType::None;
        self.drift_score = 0.0;
        self.sample_count = 0;
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, DriftType, f64, usize) {
        (
            self.sample_count,
            self.current_drift,
            self.drift_score,
            self.drift_history.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adwin_basic() {
        let mut adwin = ADWIN::new(0.002, 200);

        // Feed consistent data
        for i in 0..100 {
            let (drift, _) = adwin.update(100.0 + rand::random::<f64>() * 5.0);
            assert_eq!(drift, DriftType::None, "Should not detect drift in warm-up");
        }

        let (ref_size, curr_size, _) = adwin.get_stats();
        assert!(ref_size > 0, "Should have reference window");
        assert!(curr_size > 0, "Should have current window");
    }

    #[test]
    fn test_adwin_detects_sudden_drift() {
        let mut adwin = ADWIN::new(0.01, 200);

        // Normal data
        for _ in 0..100 {
            adwin.update(100.0);
        }

        // Sudden shift
        let mut detected = false;
        for _ in 0..50 {
            let (drift, score) = adwin.update(200.0); // Sudden shift
            if drift == DriftType::Sudden {
                detected = true;
                assert!(score > 0.0, "Should have positive drift score");
                break;
            }
        }

        assert!(detected, "ADWIN should detect sudden drift");
    }

    #[test]
    fn test_page_hinkley_gradual() {
        let mut ph = PageHinkley::new(30.0, 0.05, 0.01);

        // Normal data
        for _ in 0..50 {
            let (drift, _) = ph.update(100.0);
            assert_eq!(drift, DriftType::None);
        }

        // Gradual increase
        let mut detected = false;
        for i in 0..100 {
            let value = 100.0 + i as f64 * 1.0; // Gradual increase
            let (drift, _) = ph.update(value);
            if drift == DriftType::Gradual {
                detected = true;
                break;
            }
        }

        assert!(detected, "Page-Hinkley should detect gradual drift");
    }

    #[test]
    fn test_kl_divergence() {
        let mut kl = KLDivergenceDetector::new(20, 0.0, 100.0, 0.3);

        // Fill reference distribution
        for _ in 0..1000 {
            kl.update(50.0 + rand::random::<f64>() * 10.0);
        }

        // Continue with similar distribution (no drift)
        for _ in 0..100 {
            let (drift, _) = kl.update(50.0 + rand::random::<f64>() * 10.0);
            assert_eq!(
                drift,
                DriftType::None,
                "Should not detect drift with same distribution"
            );
        }

        // Different distribution
        let mut detected = false;
        for _ in 0..1000 {
            let (drift, _) = kl.update(80.0 + rand::random::<f64>() * 10.0); // Shifted
            if drift == DriftType::Incremental {
                detected = true;
                break;
            }
        }

        assert!(detected, "KL divergence should detect distribution shift");
    }

    #[test]
    fn test_ensemble_detector() {
        let mut ensemble = EnsembleDriftDetector::new();

        // Normal data
        for i in 0..200 {
            let (drift, _) = ensemble.update(100.0 + (i % 10) as f64);
            assert_eq!(drift, DriftType::None);
        }

        // Sudden drift
        let mut detected = false;
        for _ in 0..50 {
            let (drift, score) = ensemble.update(200.0);
            if drift != DriftType::None {
                detected = true;
                assert!(score > 0.0);
                break;
            }
        }

        assert!(detected, "Ensemble should detect drift");
        assert!(ensemble.drift_detected());
    }

    #[test]
    fn test_drift_history() {
        let mut detector = EnsembleDriftDetector::new();

        // Generate multiple drifts
        for _ in 0..3 {
            // Normal
            for _ in 0..200 {
                detector.update(100.0);
            }
            // Drift
            for _ in 0..50 {
                detector.update(200.0);
            }
        }

        let history = detector.get_history();
        assert!(!history.is_empty(), "Should have drift history");
    }
}
