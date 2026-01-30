use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Adaptive Threshold Calculator using Online Percentiles
///
/// Replaces fixed thresholds (like 20%, 5x, 10x) with data-driven
/// adaptive thresholds that adjust based on observed data distribution.
///
/// Uses multiple methods:
/// 1. EWMA + EWMVar for normal distribution assumption
/// 2. Online percentile tracking for non-parametric thresholds  
/// 3. MAD (Median Absolute Deviation) for robust statistics
#[derive(Serialize, Deserialize, Clone)]
pub struct AdaptiveThreshold {
    // Method selection
    method: ThresholdMethod,

    // EWMA-based parameters
    ewma_mean: f64,
    ewma_var: f64,
    alpha: f64,

    // Percentile-based parameters
    percentile_window: VecDeque<f64>,
    window_size: usize,
    target_percentile: f64, // e.g., 0.95 for 95th percentile

    // MAD-based parameters
    mad_history: VecDeque<f64>,
    mad_factor: f64, // Typically 3.0 for 3-sigma equivalent

    // Current threshold value
    current_threshold: f64,

    // Statistics
    update_count: u64,
    min_threshold: f64,
    max_threshold: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ThresholdMethod {
    /// EWMA mean + k*std_dev
    EwmaSigma { sigma_multiplier: f64 },
    /// Non-parametric percentile-based
    Percentile,
    /// Median Absolute Deviation (robust to outliers)
    Mad,
    /// Ensemble of all methods
    Ensemble,
}

impl AdaptiveThreshold {
    /// Create new adaptive threshold calculator
    ///
    /// # Arguments
    /// * `window_size` - Size of sliding window for percentile/MAD calculation
    /// * `alpha` - EWMA smoothing factor (0.0 to 1.0, higher = more responsive)
    /// * `method` - Threshold calculation method
    pub fn new(window_size: usize, alpha: f64, method: ThresholdMethod) -> Self {
        let ws = window_size.max(10);
        let a = alpha.clamp(0.01, 0.5);

        Self {
            method,
            ewma_mean: 0.0,
            ewma_var: 0.0,
            alpha: a,
            percentile_window: VecDeque::with_capacity(ws),
            window_size: ws,
            target_percentile: 0.95,
            mad_history: VecDeque::with_capacity(ws),
            mad_factor: 3.0,
            current_threshold: 0.0,
            update_count: 0,
            min_threshold: 0.001,
            max_threshold: f64::MAX,
        }
    }

    /// Create with EWMA sigma method (most common)
    pub fn ewma_sigma(window_size: usize, sigma_multiplier: f64) -> Self {
        let alpha = 2.0 / (window_size as f64 + 1.0);
        Self::new(
            window_size,
            alpha,
            ThresholdMethod::EwmaSigma {
                sigma_multiplier: sigma_multiplier.max(1.0),
            },
        )
    }

    /// Create with percentile method
    pub fn percentile(window_size: usize, target_percentile: f64) -> Self {
        let mut at = Self::new(window_size, 0.1, ThresholdMethod::Percentile);
        at.target_percentile = target_percentile.clamp(0.5, 0.999);
        at
    }

    /// Create with MAD method (robust to outliers)
    pub fn mad(window_size: usize, mad_factor: f64) -> Self {
        let mut at = Self::new(window_size, 0.1, ThresholdMethod::Mad);
        at.mad_factor = mad_factor.max(1.0);
        at
    }

    /// Create ensemble method (combines all approaches)
    pub fn ensemble(window_size: usize) -> Self {
        Self::new(window_size, 0.1, ThresholdMethod::Ensemble)
    }

    /// Update with new value and return current threshold
    pub fn update(&mut self, value: f64) -> f64 {
        self.update_count += 1;

        // Update EWMA statistics
        self.update_ewma(value);

        // Update window-based statistics
        self.update_windows(value);

        // Calculate threshold based on method
        self.current_threshold = match self.method {
            ThresholdMethod::EwmaSigma { sigma_multiplier } => {
                self.calculate_ewma_threshold(sigma_multiplier)
            }
            ThresholdMethod::Percentile => self.calculate_percentile_threshold(),
            ThresholdMethod::Mad => self.calculate_mad_threshold(),
            ThresholdMethod::Ensemble => self.calculate_ensemble_threshold(),
        };

        // Apply bounds
        self.current_threshold = self
            .current_threshold
            .max(self.min_threshold)
            .min(self.max_threshold);

        self.current_threshold
    }

    /// Check if value exceeds threshold
    pub fn is_anomaly(&self, value: f64) -> bool {
        value > self.current_threshold
    }

    /// Get anomaly score (0.0 = normal, 1.0+ = anomalous)
    pub fn anomaly_score(&self, value: f64) -> f64 {
        if self.current_threshold <= 0.0 {
            return 0.0;
        }

        let ratio = value / self.current_threshold;
        if ratio <= 1.0 {
            0.0
        } else {
            // Score increases as value exceeds threshold
            // Cap at 2.0 for 2x threshold
            ((ratio - 1.0).min(2.0)) / 2.0
        }
    }

    /// Update EWMA statistics
    fn update_ewma(&mut self, value: f64) {
        if self.update_count == 1 {
            self.ewma_mean = value;
            self.ewma_var = 0.0;
        } else {
            let diff = value - self.ewma_mean;
            let incr = self.alpha * diff;
            self.ewma_mean += incr;
            // Standard EWMVar update
            self.ewma_var = (1.0 - self.alpha) * (self.ewma_var + self.alpha * diff * diff);
        }
    }

    /// Update window-based data structures
    fn update_windows(&mut self, value: f64) {
        // Update percentile window
        self.percentile_window.push_back(value);
        if self.percentile_window.len() > self.window_size {
            self.percentile_window.pop_front();
        }

        // Update MAD history (track deviations from median)
        if !self.percentile_window.is_empty() {
            let median = self.calculate_median_deque(&self.percentile_window);
            let deviation = (value - median).abs();
            self.mad_history.push_back(deviation);
            if self.mad_history.len() > self.window_size {
                self.mad_history.pop_front();
            }
        }
    }

    /// Calculate threshold using EWMA + sigma
    fn calculate_ewma_threshold(&self, sigma_multiplier: f64) -> f64 {
        let std_dev = self.ewma_var.sqrt().max(self.min_threshold);
        self.ewma_mean + sigma_multiplier * std_dev
    }

    /// Calculate threshold using percentile method
    fn calculate_percentile_threshold(&self) -> f64 {
        if self.percentile_window.len() < 10 {
            return self.ewma_mean * 2.0; // Fallback during warm-up
        }

        let mut sorted: Vec<f64> = self.percentile_window.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let idx =
            ((self.target_percentile * (sorted.len() - 1) as f64) as usize).min(sorted.len() - 1);

        sorted[idx]
    }

    /// Calculate threshold using MAD (robust statistic)
    fn calculate_mad_threshold(&self) -> f64 {
        if self.mad_history.len() < 10 {
            return self.ewma_mean * 2.0; // Fallback during warm-up
        }

        let median = self.calculate_median_deque(&self.percentile_window);
        let mad = self.calculate_median_deque(&self.mad_history);

        // MAD * 1.4826 ≈ standard deviation for normal distribution
        let robust_std = mad * 1.4826;

        median + self.mad_factor * robust_std
    }

    /// Calculate ensemble threshold (conservative combination)
    fn calculate_ensemble_threshold(&self) -> f64 {
        let ewma_thresh = self.calculate_ewma_threshold(3.0);
        let percentile_thresh = self.calculate_percentile_threshold();
        let mad_thresh = self.calculate_mad_threshold();

        // Use median of the three methods (robust consensus)
        let mut thresholds = vec![ewma_thresh, percentile_thresh, mad_thresh];
        thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap());

        thresholds[1] // Median
    }

    /// Calculate median of a slice
    fn calculate_median(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<f64> = data.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = sorted.len();
        if n % 2 == 0 {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        } else {
            sorted[n / 2]
        }
    }

    /// Calculate median from VecDeque
    fn calculate_median_deque(&self, deque: &std::collections::VecDeque<f64>) -> f64 {
        let slice: Vec<f64> = deque.iter().copied().collect();
        self.calculate_median(&slice)
    }

    /// Get current statistics
    pub fn get_stats(&self) -> (f64, f64, f64, u64) {
        (
            self.ewma_mean,
            self.ewma_var.sqrt(),
            self.current_threshold,
            self.update_count,
        )
    }

    /// Set minimum threshold (prevents thresholds from going too low)
    pub fn set_min_threshold(&mut self, min: f64) {
        self.min_threshold = min.max(0.0);
    }

    /// Set maximum threshold (prevents thresholds from going too high)
    pub fn set_max_threshold(&mut self, max: f64) {
        self.max_threshold = max.max(self.min_threshold);
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.ewma_mean = 0.0;
        self.ewma_var = 0.0;
        self.percentile_window.clear();
        self.mad_history.clear();
        self.current_threshold = 0.0;
        self.update_count = 0;
    }
}

/// Pre-configured threshold presets for common use cases
pub mod presets {
    use super::*;

    /// For volume/RPS detection (responsive, 2-sigma)
    pub fn volume_threshold() -> AdaptiveThreshold {
        AdaptiveThreshold::ewma_sigma(50, 2.0)
    }

    /// For distribution/latency detection (conservative, 3-sigma)
    pub fn distribution_threshold() -> AdaptiveThreshold {
        AdaptiveThreshold::ewma_sigma(100, 3.0)
    }

    /// For cardinality detection (percentile-based, 95th)
    pub fn cardinality_threshold() -> AdaptiveThreshold {
        AdaptiveThreshold::percentile(100, 0.95)
    }

    /// For burst detection (MAD-based, robust to outliers)
    pub fn burst_threshold() -> AdaptiveThreshold {
        AdaptiveThreshold::mad(50, 3.0)
    }

    /// Conservative ensemble (all methods)
    pub fn conservative_threshold() -> AdaptiveThreshold {
        AdaptiveThreshold::ensemble(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_sigma_threshold() {
        let mut threshold = AdaptiveThreshold::ewma_sigma(20, 2.0);

        // Warm up with normal data around 100
        for _ in 0..30 {
            threshold.update(100.0 + rand::random::<f64>() * 5.0);
        }

        let thresh_val = threshold.current_threshold;
        let (mean, std, _, _) = threshold.get_stats();

        // Threshold should be approximately mean + 2*std
        assert!(thresh_val > mean, "Threshold should be above mean");
        assert!(
            (thresh_val - (mean + 2.0 * std)).abs() < 5.0,
            "Threshold should be mean + 2*sigma"
        );
    }

    #[test]
    fn test_percentile_threshold() {
        let mut threshold = AdaptiveThreshold::percentile(50, 0.90);

        // Add values 1 to 100
        for i in 1..=100 {
            threshold.update(i as f64);
        }

        // 90th percentile of 1-100 should be around 90
        assert!(
            threshold.current_threshold >= 85.0,
            "90th percentile threshold should be high: got {}",
            threshold.current_threshold
        );
        assert!(
            threshold.current_threshold <= 95.0,
            "90th percentile threshold should not exceed range"
        );
    }

    #[test]
    fn test_mad_threshold() {
        let mut threshold = AdaptiveThreshold::mad(50, 3.0);

        // Normal data
        for _ in 0..50 {
            threshold.update(100.0 + rand::random::<f64>() * 2.0);
        }

        let (mean, _, thresh, _) = threshold.get_stats();

        // Should detect outliers
        assert!(threshold.is_anomaly(150.0), "Should detect 5-sigma outlier");
        assert!(!threshold.is_anomaly(mean), "Should not flag mean value");
    }

    #[test]
    fn test_anomaly_score() {
        let mut threshold = AdaptiveThreshold::ewma_sigma(20, 2.0);

        // Warm up
        for _ in 0..25 {
            threshold.update(10.0);
        }

        let thresh = threshold.current_threshold;

        // Normal value should have score 0
        let score_normal = threshold.anomaly_score(thresh * 0.5);
        assert_eq!(score_normal, 0.0, "Normal value should have score 0");

        // At threshold should have score 0
        let score_at = threshold.anomaly_score(thresh);
        assert_eq!(score_at, 0.0, "At threshold should have score 0");

        // 2x threshold should have score 0.5
        let score_2x = threshold.anomaly_score(thresh * 2.0);
        assert!(
            (score_2x - 0.5).abs() < 0.01,
            "2x threshold should have score ~0.5"
        );
    }

    #[test]
    fn test_presets() {
        let volume = presets::volume_threshold();
        let dist = presets::distribution_threshold();
        let card = presets::cardinality_threshold();
        let burst = presets::burst_threshold();

        // Just verify they create successfully
        assert!(matches!(volume.method, ThresholdMethod::EwmaSigma { .. }));
        assert!(matches!(dist.method, ThresholdMethod::EwmaSigma { .. }));
        assert!(matches!(card.method, ThresholdMethod::Percentile));
        assert!(matches!(burst.method, ThresholdMethod::Mad));
    }

    #[test]
    fn test_adaptation() {
        let mut threshold = AdaptiveThreshold::ewma_sigma(30, 2.0);

        // Phase 1: Low values
        for _ in 0..40 {
            threshold.update(10.0);
        }
        let thresh_low = threshold.current_threshold;

        // Phase 2: Shift to higher values
        for _ in 0..40 {
            threshold.update(100.0);
        }
        let thresh_high = threshold.current_threshold;

        // Threshold should adapt upward
        assert!(
            thresh_high > thresh_low * 5.0,
            "Threshold should adapt to higher values: low={}, high={}",
            thresh_low,
            thresh_high
        );
    }
}
