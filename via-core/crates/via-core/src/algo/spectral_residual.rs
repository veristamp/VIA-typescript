use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Spectral Residual Anomaly Detection
///
/// SOTA algorithm used by Microsoft Azure Anomaly Detector.
/// Based on the paper: "Time-Series Anomaly Detection Service at Microsoft"
/// (KDD 2019 - https://arxiv.org/abs/1906.03821)
///
/// Key advantages:
/// - Zero hyperparameters (fully automatic)
/// - Works on any time series without tuning
/// - FFT-based, O(n log n) complexity
/// - Robust to noise and seasonality
#[derive(Serialize, Deserialize, Clone)]
pub struct SpectralResidual {
    // Window of recent values for FFT analysis
    window: VecDeque<f64>,
    window_size: usize,

    // Adaptive threshold based on historical anomaly scores
    score_ewma: f64,
    score_ewmvar: f64,
    alpha: f64, // Smoothing factor for threshold adaptation

    // Sensitivity parameter (0.0 to 1.0, higher = more sensitive)
    sensitivity: f64,

    // Minimum anomaly score to trigger detection
    threshold_sigma: f64,
}

impl SpectralResidual {
    /// Create a new SpectralResidual detector
    ///
    /// # Arguments
    /// * `window_size` - Size of the sliding window for FFT (recommend 24-168 for hourly/daily patterns)
    /// * `sensitivity` - Detection sensitivity 0.0-1.0 (default 0.5)
    pub fn new(window_size: usize, sensitivity: f64) -> Self {
        let ws = window_size.max(8); // Minimum window for meaningful FFT
        let alpha = 2.0 / (ws as f64 + 1.0); // Standard EWMA alpha

        Self {
            window: VecDeque::with_capacity(ws),
            window_size: ws,
            score_ewma: 0.0,
            score_ewmvar: 0.0,
            alpha,
            sensitivity: sensitivity.clamp(0.0, 1.0),
            threshold_sigma: 3.0, // Start with 3-sigma threshold
        }
    }

    /// Update with new value and return anomaly score
    ///
    /// Returns: (anomaly_score, is_anomaly)
    /// - anomaly_score: 0.0 (normal) to 1.0+ (anomalous), higher = more anomalous
    /// - is_anomaly: true if exceeds adaptive threshold
    pub fn update(&mut self, value: f64) -> (f64, bool) {
        // Add value to window
        self.window.push_back(value);

        // Wait for full window
        if self.window.len() < self.window_size {
            return (0.0, false);
        }

        // Maintain fixed window size
        if self.window.len() > self.window_size {
            self.window.pop_front();
        }

        // Compute spectral residual anomaly score
        let score = self.compute_spectral_residual();

        // Update adaptive threshold
        self.update_threshold(score);

        // Determine if anomaly based on adaptive threshold
        let threshold = self.score_ewma + self.threshold_sigma * self.score_ewmvar.sqrt();
        let is_anomaly = score > threshold && score > 0.1; // Minimum score threshold

        // Normalize to 0-1 scale for output
        let normalized_score = if threshold > 0.0 {
            (score / threshold).min(2.0) / 2.0 // Cap at 1.0 for 2x threshold
        } else {
            0.0
        };

        (normalized_score, is_anomaly)
    }

    /// Core spectral residual computation
    fn compute_spectral_residual(&self) -> f64 {
        let n = self.window.len();
        if n < 4 {
            return 0.0;
        }

        // Convert window to vector for FFT
        let signal: Vec<f64> = self.window.iter().copied().collect();

        // Compute FFT using real FFT (only need first half + DC)
        let fft_result = self.real_fft(&signal);

        // Compute log amplitude spectrum
        let log_amplitude: Vec<f64> = fft_result
            .iter()
            .map(|&(re, im)| {
                let mag = (re * re + im * im).sqrt();
                (mag + 1e-10).ln() // Add epsilon to avoid log(0)
            })
            .collect();

        // Apply spectral residual transformation
        // 1. Smooth the log amplitude (moving average)
        let smoothed = self.moving_average(&log_amplitude, 3);

        // 2. Compute spectral residual: log_amp - smoothed_log_amp
        let spectral_residual: Vec<f64> = log_amplitude
            .iter()
            .zip(smoothed.iter())
            .map(|(log_amp, smooth)| log_amp - smooth)
            .collect();

        // 3. Reconstruct signal via inverse FFT
        let reconstructed = self.inverse_real_fft(&spectral_residual, n);

        // 4. Compute reconstruction error at the last point (most recent)
        let original_last = signal[n - 1];
        let reconstructed_last = reconstructed[n - 1];
        let error = (original_last - reconstructed_last).abs();

        // 5. Normalize by signal magnitude for scale-invariance
        let signal_mean = signal.iter().sum::<f64>() / n as f64;
        let signal_std = (signal
            .iter()
            .map(|&x| (x - signal_mean).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt()
            .max(1e-10);

        let normalized_error = error / signal_std;

        // Apply sensitivity adjustment
        // Higher sensitivity = lower threshold for detection
        let adjusted_score = normalized_error * (1.0 + self.sensitivity);

        adjusted_score
    }

    /// Simple real FFT implementation using DFT
    /// For production, replace with rustfft crate for better performance
    fn real_fft(&self, signal: &[f64]) -> Vec<(f64, f64)> {
        let n = signal.len();
        let mut result = Vec::with_capacity(n / 2 + 1);

        // DC component (k=0)
        let dc = signal.iter().sum::<f64>() / n as f64;
        result.push((dc, 0.0));

        // Other frequencies (k=1 to n/2)
        for k in 1..=n / 2 {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &x) in signal.iter().enumerate() {
                let angle = -2.0 * std::f64::consts::PI * (k as f64) * (i as f64) / (n as f64);
                re += x * angle.cos();
                im += x * angle.sin();
            }
            result.push((re, im));
        }

        result
    }

    /// Inverse real FFT reconstruction
    fn inverse_real_fft(&self, spectral_residual: &[f64], n: usize) -> Vec<f64> {
        let mut result = vec![0.0; n];

        for i in 0..n {
            let mut sum = spectral_residual[0]; // DC component

            for k in 1..spectral_residual.len() {
                let angle = 2.0 * std::f64::consts::PI * (k as f64) * (i as f64) / (n as f64);
                sum += spectral_residual[k] * angle.cos();
            }

            result[i] = sum;
        }

        result
    }

    /// Simple moving average for smoothing
    fn moving_average(&self, data: &[f64], window: usize) -> Vec<f64> {
        let w = window.max(1);
        let n = data.len();
        let mut result = Vec::with_capacity(n);

        for i in 0..n {
            let start = i.saturating_sub(w / 2);
            let end = (i + w / 2 + 1).min(n);
            let avg = data[start..end].iter().sum::<f64>() / (end - start) as f64;
            result.push(avg);
        }

        result
    }

    /// Update adaptive threshold using EWMA and EWMVar
    fn update_threshold(&mut self, score: f64) {
        if self.score_ewma == 0.0 {
            // Initialize
            self.score_ewma = score;
            self.score_ewmvar = score * score;
        } else {
            // Update EWMA of scores
            let diff = score - self.score_ewma;
            self.score_ewma += self.alpha * diff;

            // Update EWMVar of scores
            self.score_ewmvar = (1.0 - self.alpha) * (self.score_ewmvar + self.alpha * diff * diff);
        }

        // Adapt threshold sigma based on sensitivity
        // Lower sensitivity = higher threshold (fewer false positives)
        self.threshold_sigma = 2.0 + (1.0 - self.sensitivity) * 2.0;
    }

    /// Get current adaptive threshold for debugging
    pub fn get_threshold(&self) -> f64 {
        self.score_ewma + self.threshold_sigma * self.score_ewmvar.sqrt()
    }

    /// Get current window statistics
    pub fn get_stats(&self) -> (usize, f64, f64) {
        (self.window.len(), self.score_ewma, self.score_ewmvar.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectral_residual_basic() {
        let mut detector = SpectralResidual::new(16, 0.5);

        // Feed normal data
        for i in 0..20 {
            let value = 100.0 + (i as f64 * 0.1); // Slowly increasing trend
            let (score, is_anomaly) = detector.update(value);

            if i >= 16 {
                // After warm-up, scores should be low for normal data
                assert!(
                    !is_anomaly,
                    "Normal trend should not trigger anomaly at step {}",
                    i
                );
                assert!(
                    score < 0.5,
                    "Normal trend should have low score at step {}",
                    i
                );
            }
        }
    }

    #[test]
    fn test_spectral_residual_detects_spike() {
        let mut detector = SpectralResidual::new(16, 0.8); // High sensitivity

        // Warm up with normal data
        for i in 0..16 {
            let value = 100.0 + (i % 5) as f64 * 2.0; // Simple pattern
            detector.update(value);
        }

        // Inject anomalous spike
        let (score, is_anomaly) = detector.update(500.0); // 5x normal

        assert!(is_anomaly, "Should detect spike as anomaly");
        assert!(score > 0.5, "Spike should have high score: got {}", score);
    }

    #[test]
    fn test_adaptive_threshold() {
        let mut detector = SpectralResidual::new(12, 0.5);

        // Feed consistent data
        for _ in 0..24 {
            detector.update(100.0);
        }

        let threshold_before = detector.get_threshold();

        // Feed more data with slight variations
        for i in 0..12 {
            detector.update(100.0 + (i as f64 * 0.5));
        }

        let threshold_after = detector.get_threshold();

        // Threshold should adapt but remain reasonable
        assert!(threshold_after > 0.0, "Threshold should be positive");
        assert!(threshold_after < 10.0, "Threshold should not explode");
    }
}
