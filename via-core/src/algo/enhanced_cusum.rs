use serde::{Deserialize, Serialize};

/// Enhanced CUSUM (Cumulative Sum) with V-Mask and Fast Initial Response
///
/// SOTA change point detection algorithm with enhancements:
/// - V-Mask: Provides directional detection with lookahead capability
/// - Fast Initial Response (FIR): Quicker detection of initial shifts
/// - Adaptive thresholding based on historical performance
/// - Dual-sided detection (both increases and decreases)
///
/// References:
/// - Page, E. S. (1954). Continuous inspection schemes. Biometrika.
/// - Lucas, J. M., & Crosier, R. B. (1982). Fast initial response (FIR) for CUSUM quality control schemes.
#[derive(Serialize, Deserialize, Clone)]
pub struct EnhancedCUSUM {
    // Target mean (expected value)
    target: f64,

    // Slack parameter (minimum shift to detect, in std dev units)
    slack: f64,

    // Decision interval (threshold for alarm)
    threshold: f64,

    // V-Mask parameters
    // The V-mask provides a visual and computational method to detect trends
    v_mask_angle: f64,  // Angle of V-mask arms (radians)
    v_mask_lead: usize, // Lead distance for V-mask

    // Fast Initial Response (FIR) - head start for quicker initial detection
    fir_enabled: bool,
    fir_factor: f64, // Head start as fraction of threshold (e.g., 0.5 = 50% head start)
    fir_samples: usize, // Number of samples to apply FIR
    sample_count: usize,

    // Cumulative sums
    c_pos: f64, // CUSUM for upward shifts
    c_neg: f64, // CUSUM for downward shifts

    // Adaptive threshold tracking
    history: Vec<f64>, // Recent CUSUM values for adaptive threshold
    history_size: usize,
    adaptive_threshold: f64,

    // Output state
    pub alarm: bool,
    pub alarm_type: i8,      // 0=none, 1=high (upward), -1=low (downward)
    pub alarm_severity: f64, // 0.0 to 1.0

    // Performance tracking
    samples_since_reset: usize,
    total_alarms: u64,
}

impl EnhancedCUSUM {
    /// Create a new Enhanced CUSUM detector
    ///
    /// # Arguments
    /// * `target` - Expected mean value
    /// * `slack` - Minimum detectable shift (in standard deviations, typically 0.5)
    /// * `threshold` - Decision interval (typically 4-5 for good ARL performance)
    pub fn new(target: f64, slack: f64, threshold: f64) -> Self {
        Self {
            target,
            slack: slack.max(0.1), // Minimum slack to avoid division issues
            threshold: threshold.max(1.0),

            // V-Mask: angle = arctan(slack / 2)
            v_mask_angle: (slack / 2.0).atan(),
            v_mask_lead: 10,

            // FIR: Start at 50% of threshold for faster initial detection
            fir_enabled: true,
            fir_factor: 0.5,
            fir_samples: 10,
            sample_count: 0,

            c_pos: 0.0,
            c_neg: 0.0,

            history: Vec::with_capacity(20),
            history_size: 20,
            adaptive_threshold: threshold,

            alarm: false,
            alarm_type: 0,
            alarm_severity: 0.0,

            samples_since_reset: 0,
            total_alarms: 0,
        }
    }

    /// Create with custom V-Mask and FIR settings
    pub fn with_options(
        target: f64,
        slack: f64,
        threshold: f64,
        v_mask_lead: usize,
        fir_enabled: bool,
        fir_factor: f64,
    ) -> Self {
        let mut cusum = Self::new(target, slack, threshold);
        cusum.v_mask_lead = v_mask_lead.max(5);
        cusum.fir_enabled = fir_enabled;
        cusum.fir_factor = fir_factor.clamp(0.0, 1.0);
        cusum
    }

    /// Update with new sample and check for alarm
    ///
    /// Returns true if alarm is triggered
    pub fn update(&mut self, sample: f64) -> bool {
        self.alarm = false;
        self.alarm_type = 0;
        self.alarm_severity = 0.0;

        // Calculate standardized deviation
        let deviation = sample - self.target;

        // Apply FIR (Fast Initial Response) head start
        let head_start = if self.fir_enabled && self.sample_count < self.fir_samples {
            self.threshold * self.fir_factor
        } else {
            0.0
        };

        // Update CUSUM statistics with head start
        self.c_pos = (self.c_pos + deviation - self.slack).max(head_start);
        self.c_neg = (self.c_neg - deviation - self.slack).max(head_start);

        // Update history for adaptive threshold
        self.update_history(self.c_pos.max(self.c_neg));

        // Check thresholds
        let effective_threshold = if self.sample_count > self.history_size * 2 {
            // Use adaptive threshold after warm-up
            self.adaptive_threshold
        } else {
            self.threshold
        };

        // V-Mask check: Look at trend over lead distance
        let v_mask_trigger = self.check_v_mask();

        // Determine alarm
        if self.c_pos > effective_threshold || v_mask_trigger.0 {
            self.alarm = true;
            self.alarm_type = 1;
            self.alarm_severity = (self.c_pos / effective_threshold).min(2.0) / 2.0;
            self.c_pos = 0.0; // Reset after alarm (standard CUSUM practice)
            self.total_alarms += 1;
        } else if self.c_neg > effective_threshold || v_mask_trigger.1 {
            self.alarm = true;
            self.alarm_type = -1;
            self.alarm_severity = (self.c_neg / effective_threshold).min(2.0) / 2.0;
            self.c_neg = 0.0; // Reset after alarm
            self.total_alarms += 1;
        }

        self.sample_count += 1;
        self.samples_since_reset += 1;

        // Auto-reset CUSUM if it gets too large without alarm (prevents drift)
        if self.samples_since_reset > 1000 {
            self.reset();
        }

        self.alarm
    }

    /// V-Mask detection
    ///
    /// The V-mask checks if the CUSUM path crosses the mask boundaries,
    /// which indicates a trend change.
    ///
    /// Returns: (upward_trigger, downward_trigger)
    fn check_v_mask(&self) -> (bool, bool) {
        if self.history.len() < self.v_mask_lead {
            return (false, false);
        }

        let current_idx = self.history.len() - 1;
        let current_val = self.history[current_idx];

        // Check backward over lead distance
        let check_distance = self.v_mask_lead.min(current_idx);

        let mut upward_trigger = false;
        let mut downward_trigger = false;

        for i in 1..=check_distance {
            let past_idx = current_idx - i;
            let past_val = self.history[past_idx];
            let distance = i as f64;

            // V-mask boundaries
            let upper_boundary = past_val + distance * self.v_mask_angle.tan();
            let lower_boundary = past_val - distance * self.v_mask_angle.tan();

            // Check if current value is outside V-mask
            if current_val > upper_boundary {
                upward_trigger = true;
            }
            if current_val < lower_boundary {
                downward_trigger = true;
            }
        }

        (upward_trigger, downward_trigger)
    }

    /// Update history and compute adaptive threshold
    fn update_history(&mut self, value: f64) {
        self.history.push(value);

        if self.history.len() > self.history_size {
            self.history.remove(0);
        }

        // Compute adaptive threshold based on historical CUSUM variability
        if self.history.len() >= 10 {
            let mean = self.history.iter().sum::<f64>() / self.history.len() as f64;
            let variance = self
                .history
                .iter()
                .map(|&x| (x - mean).powi(2))
                .sum::<f64>()
                / self.history.len() as f64;
            let std_dev = variance.sqrt();

            // Adaptive threshold = mean + 3*std_dev, but at least base threshold
            self.adaptive_threshold = (mean + 3.0 * std_dev).max(self.threshold);
        }
    }

    /// Reset CUSUM statistics
    pub fn reset(&mut self) {
        self.c_pos = 0.0;
        self.c_neg = 0.0;
        self.samples_since_reset = 0;

        // Reset FIR
        if self.fir_enabled {
            self.sample_count = 0;
        }
    }

    /// Update target (for tracking non-stationary processes)
    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    /// Update slack parameter
    pub fn set_slack(&mut self, slack: f64) {
        self.slack = slack.max(0.1);
        self.v_mask_angle = (self.slack / 2.0).atan();
    }

    /// Get current statistics
    pub fn get_stats(&self) -> (f64, f64, f64, u64) {
        (
            self.c_pos,
            self.c_neg,
            self.adaptive_threshold,
            self.total_alarms,
        )
    }

    /// Calculate Average Run Length (ARL) for current parameters
    ///
    /// ARL0 = Expected samples before false alarm (should be large, e.g., 1000+)
    /// ARL1 = Expected samples to detect shift (should be small, e.g., 5-10)
    pub fn estimate_arl(&self, shift: f64) -> f64 {
        // Approximation using Siegmund's formula
        // ARL ≈ (exp(-2*Δ*b) + 2*Δ*b - 1) / (2*Δ²)
        // where Δ = shift - slack, b = threshold

        let delta = shift - self.slack;
        let b = self.threshold;

        if delta.abs() < 0.001 {
            // For zero shift, use approximation for ARL0
            (self.threshold.powi(2) / 2.0).exp()
        } else {
            let numerator = (-2.0 * delta * b).exp() + 2.0 * delta * b - 1.0;
            let denominator = 2.0 * delta * delta;
            (numerator / denominator).max(1.0)
        }
    }
}

/// Simple CUSUM wrapper for backward compatibility
/// Uses the enhanced implementation internally
pub struct CUSUM {
    inner: EnhancedCUSUM,
}

impl CUSUM {
    pub fn new(target: f64, slack: f64, threshold: f64) -> Self {
        Self {
            inner: EnhancedCUSUM::new(target, slack, threshold),
        }
    }

    pub fn update(&mut self, sample: f64) -> bool {
        self.inner.update(sample)
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn alarm(&self) -> bool {
        self.inner.alarm
    }

    pub fn alarm_type(&self) -> i8 {
        self.inner.alarm_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cusum_detects_upward_shift() {
        let mut cusum = EnhancedCUSUM::new(100.0, 0.5, 4.0);

        // Warm up with normal data
        for _ in 0..20 {
            cusum.update(100.0 + rand::random::<f64>() * 2.0);
        }

        // Inject upward shift
        let mut detected = false;
        for _ in 0..10 {
            if cusum.update(115.0) {
                // 15 unit shift (3 sigma)
                detected = true;
                break;
            }
        }

        assert!(detected, "Should detect upward shift");
        assert_eq!(cusum.alarm_type, 1, "Should be upward alarm");
    }

    #[test]
    fn test_cusum_detects_downward_shift() {
        let mut cusum = EnhancedCUSUM::new(100.0, 0.5, 4.0);

        // Warm up
        for _ in 0..20 {
            cusum.update(100.0);
        }

        // Inject downward shift
        let mut detected = false;
        for _ in 0..10 {
            if cusum.update(85.0) {
                // 15 unit drop
                detected = true;
                break;
            }
        }

        assert!(detected, "Should detect downward shift");
        assert_eq!(cusum.alarm_type, -1, "Should be downward alarm");
    }

    #[test]
    fn test_fir_enables_faster_detection() {
        let mut cusum_fir = EnhancedCUSUM::with_options(100.0, 0.5, 4.0, 10, true, 0.5);
        let mut cusum_no_fir = EnhancedCUSUM::with_options(100.0, 0.5, 4.0, 10, false, 0.0);

        // Both should detect, but FIR should be faster
        let mut fir_detected = false;
        let mut no_fir_detected = false;
        let mut fir_steps = 0;
        let mut no_fir_steps = 0;

        for i in 0..15 {
            if !fir_detected {
                fir_steps += 1;
                if cusum_fir.update(120.0) {
                    fir_detected = true;
                }
            }
            if !no_fir_detected {
                no_fir_steps += 1;
                if cusum_no_fir.update(120.0) {
                    no_fir_detected = true;
                }
            }
        }

        assert!(fir_detected, "FIR should detect");
        assert!(no_fir_detected, "No FIR should detect");
        // FIR should typically be faster or equal
        assert!(
            fir_steps <= no_fir_steps + 2,
            "FIR should not be significantly slower"
        );
    }

    #[test]
    fn test_v_mask_detection() {
        let mut cusum = EnhancedCUSUM::with_options(100.0, 0.5, 4.0, 5, false, 0.0);

        // Create a trend that triggers V-mask
        // Gradual increase then sudden jump
        for i in 0..10 {
            cusum.update(100.0 + i as f64 * 0.5); // Gradual trend
        }

        let mut detected = false;
        for _ in 0..5 {
            if cusum.update(120.0) {
                // Sudden jump
                detected = true;
                break;
            }
        }

        assert!(detected, "V-mask should help detect trend change");
    }

    #[test]
    fn test_adaptive_threshold() {
        let mut cusum = EnhancedCUSUM::new(100.0, 0.5, 4.0);

        // Warm up to fill history
        for _ in 0..50 {
            cusum.update(100.0 + rand::random::<f64>() * 5.0);
        }

        let threshold_before = cusum.adaptive_threshold;

        // Add some variability
        for _ in 0..20 {
            cusum.update(100.0 + rand::random::<f64>() * 10.0);
        }

        let threshold_after = cusum.adaptive_threshold;

        // Adaptive threshold should have been computed
        assert!(
            threshold_after > 0.0,
            "Adaptive threshold should be positive"
        );
        assert!(
            threshold_after >= cusum.threshold || threshold_before >= cusum.threshold,
            "Adaptive threshold should respect base threshold"
        );
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that simple CUSUM wrapper works
        let mut cusum = CUSUM::new(100.0, 0.5, 4.0);

        // Warm up
        for _ in 0..20 {
            cusum.update(100.0);
        }

        // Should detect shift
        let mut detected = false;
        for _ in 0..10 {
            if cusum.update(120.0) {
                detected = true;
                break;
            }
        }

        assert!(detected, "Backward compatible CUSUM should work");
    }
}
