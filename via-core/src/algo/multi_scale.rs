//! Multi-Scale Temporal Analysis Detector
//!
//! Detects anomalies at different time granularities:
//! - Second-level: Immediate anomalies (microbursts)
//! - Minute-level: Short-term trends
//! - Hour-level: Daily patterns (Fourier analysis)
//! - Day-level: Weekly patterns (seasonal decomposition)
//!
//! This provides a comprehensive view of temporal anomalies across scales.

use crate::algo::ewma::EWMA;
use crate::algo::holtwinters::HoltWinters;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Time scale for analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeScale {
    Second,
    Minute,
    Hour,
    Day,
}

/// Detector for a specific time scale
#[derive(Serialize, Deserialize, Clone)]
struct ScaleDetector {
    scale: TimeScale,
    /// EWMA for short-term tracking
    ewma: EWMA,
    /// Holt-Winters for trend/seasonality
    hw: Option<HoltWinters>,
    /// Value buffer for Fourier analysis
    value_buffer: VecDeque<f64>,
    /// Buffer size (depends on scale)
    buffer_size: usize,
    /// Last update timestamp
    last_update_ns: u64,
    /// Window size in nanoseconds
    window_ns: u64,
    /// Cumulative value in current window
    window_sum: f64,
    /// Count in current window
    window_count: u64,
    /// Seasonality detected flag
    has_seasonality: bool,
    /// Fourier coefficients (computed periodically)
    fourier_coeffs: Vec<(f64, f64)>,
}

impl ScaleDetector {
    fn new(scale: TimeScale) -> Self {
        let (buffer_size, window_ns, ewma_halflife) = match scale {
            TimeScale::Second => (10, 1_000_000_000u64, 5.0), // 1 second
            TimeScale::Minute => (60, 60_000_000_000u64, 10.0), // 1 minute
            TimeScale::Hour => (24, 3_600_000_000_000u64, 50.0), // 1 hour
            TimeScale::Day => (7, 86_400_000_000_000u64, 100.0), // 1 day
        };

        let hw = if scale == TimeScale::Hour || scale == TimeScale::Day {
            // Use Holt-Winters for longer scales
            Some(HoltWinters::new(
                0.3,
                0.1,
                0.1,
                match scale {
                    TimeScale::Hour => 24, // Daily pattern within hours
                    TimeScale::Day => 7,   // Weekly pattern
                    _ => 4,
                },
            ))
        } else {
            None
        };

        Self {
            scale,
            ewma: EWMA::new(ewma_halflife),
            hw,
            value_buffer: VecDeque::with_capacity(buffer_size),
            buffer_size,
            last_update_ns: 0,
            window_ns,
            window_sum: 0.0,
            window_count: 0,
            has_seasonality: false,
            fourier_coeffs: vec![],
        }
    }

    /// Update with new value at given timestamp
    fn update(&mut self, value: f64, timestamp_ns: u64) -> Option<(f64, f64, bool)> {
        // Check if we need to emit a windowed value
        let window_elapsed = if self.last_update_ns == 0 {
            0
        } else {
            timestamp_ns.saturating_sub(self.last_update_ns)
        };

        if window_elapsed >= self.window_ns && self.window_count > 0 {
            // Emit window average
            let window_avg = self.window_sum / self.window_count as f64;
            let result = self.process_windowed_value(window_avg);

            // Reset window
            self.window_sum = value;
            self.window_count = 1;
            self.last_update_ns = timestamp_ns;

            return Some(result);
        }

        // Accumulate in window
        self.window_sum += value;
        self.window_count += 1;

        if self.last_update_ns == 0 {
            self.last_update_ns = timestamp_ns;
        }

        None
    }

    /// Process a windowed (aggregated) value
    fn process_windowed_value(&mut self, value: f64) -> (f64, f64, bool) {
        // Update EWMA
        let ewma_val = self.ewma.update(value);

        // Update Holt-Winters if available
        let (prediction, deviation) = if let Some(ref mut hw) = self.hw {
            hw.update(value)
        } else {
            (ewma_val, (value - ewma_val).abs())
        };

        // Update buffer for Fourier analysis
        self.value_buffer.push_back(value);
        if self.value_buffer.len() > self.buffer_size {
            self.value_buffer.pop_front();
        }

        // Periodically compute Fourier coefficients
        if self.value_buffer.len() >= self.buffer_size && self.fourier_coeffs.is_empty() {
            self.compute_fourier();
        }

        // Calculate anomaly score based on scale
        let score = self.calculate_score(value, prediction, deviation);
        let _is_anomaly = score > 0.7;

        // Check for seasonality on longer scales
        let is_seasonal = if self.scale == TimeScale::Hour || self.scale == TimeScale::Day {
            self.detect_seasonality(value)
        } else {
            false
        };

        (score, prediction, is_seasonal)
    }

    /// Calculate anomaly score
    fn calculate_score(&self, _value: f64, prediction: f64, deviation: f64) -> f64 {
        let threshold = match self.scale {
            TimeScale::Second => prediction * 0.3, // 30% tolerance
            TimeScale::Minute => prediction * 0.2, // 20% tolerance
            TimeScale::Hour => prediction * 0.15,  // 15% tolerance
            TimeScale::Day => prediction * 0.1,    // 10% tolerance
        }
        .max(1.0);

        let excess = (deviation - threshold).max(0.0);
        let score = (excess / prediction.max(1.0)).min(2.0) / 2.0;

        score.clamp(0.0, 1.0)
    }

    /// Detect seasonality using Fourier analysis
    fn detect_seasonality(&mut self, _value: f64) -> bool {
        if self.fourier_coeffs.is_empty() || self.value_buffer.len() < self.buffer_size {
            return false;
        }

        // Simple seasonality check: dominant frequency has significant power
        let total_power: f64 = self
            .fourier_coeffs
            .iter()
            .map(|(re, im)| (re * re + im * im).sqrt())
            .sum();

        let dominant_power = self
            .fourier_coeffs
            .iter()
            .map(|(re, im)| (re * re + im * im).sqrt())
            .fold(0.0, f64::max);

        // If dominant frequency has > 40% of total power, it's seasonal
        self.has_seasonality = dominant_power > total_power * 0.4;
        self.has_seasonality
    }

    /// Compute simple DFT for seasonality detection
    fn compute_fourier(&mut self) {
        let n = self.value_buffer.len();
        if n < 4 {
            return;
        }

        let data: Vec<f64> = self.value_buffer.iter().copied().collect();
        let mut coeffs = Vec::new();

        // Compute first few DFT coefficients
        for k in 0..(n / 2).min(8) {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &x) in data.iter().enumerate() {
                let angle = -2.0 * std::f64::consts::PI * (k as f64) * (i as f64) / (n as f64);
                re += x * angle.cos();
                im += x * angle.sin();
            }
            coeffs.push((re, im));
        }

        self.fourier_coeffs = coeffs;
    }

    fn get_stats(&self) -> (usize, f64, bool) {
        (
            self.value_buffer.len(),
            self.ewma.value(),
            self.has_seasonality,
        )
    }
}

/// Multi-Scale Temporal Analysis
#[derive(Serialize, Deserialize, Clone)]
pub struct MultiScaleDetector {
    /// Detectors for each scale
    second_level: ScaleDetector,
    minute_level: ScaleDetector,
    hour_level: ScaleDetector,
    day_level: ScaleDetector,
    /// Combined anomaly score
    combined_score: f64,
    /// Scale-specific flags
    active_scales: Vec<TimeScale>,
    /// Last timestamp
    last_timestamp: u64,
    /// Sample count
    sample_count: u64,
}

/// Result from multi-scale analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiScaleResult {
    pub combined_score: f64,
    pub is_anomaly: bool,
    pub active_scales: Vec<(TimeScale, f64, bool)>, // (scale, score, has_seasonality)
    pub primary_scale: Option<TimeScale>,
    pub has_seasonality: bool,
}

impl MultiScaleDetector {
    pub fn new() -> Self {
        Self {
            second_level: ScaleDetector::new(TimeScale::Second),
            minute_level: ScaleDetector::new(TimeScale::Minute),
            hour_level: ScaleDetector::new(TimeScale::Hour),
            day_level: ScaleDetector::new(TimeScale::Day),
            combined_score: 0.0,
            active_scales: vec![],
            last_timestamp: 0,
            sample_count: 0,
        }
    }

    /// Update with new value
    pub fn update(&mut self, value: f64, timestamp_ns: u64) -> MultiScaleResult {
        self.sample_count += 1;
        self.last_timestamp = timestamp_ns;

        let mut scale_results = vec![];
        let mut max_score = 0.0;
        let mut primary_scale = None;
        let mut any_seasonality = false;

        // Update all scales
        let mut scales = [
            (&mut self.second_level, TimeScale::Second),
            (&mut self.minute_level, TimeScale::Minute),
            (&mut self.hour_level, TimeScale::Hour),
            (&mut self.day_level, TimeScale::Day),
        ];

        for (detector, scale) in scales.iter_mut() {
            if let Some((score, _prediction, is_seasonal)) = detector.update(value, timestamp_ns) {
                scale_results.push((*scale, score, is_seasonal));

                if score > max_score {
                    max_score = score;
                    primary_scale = Some(*scale);
                }

                if is_seasonal {
                    any_seasonality = true;
                }
            }
        }

        // Weighted combination (shorter scales have higher weight for immediate detection)
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (scale, score, _) in &scale_results {
            let weight = match scale {
                TimeScale::Second => 1.0,
                TimeScale::Minute => 0.8,
                TimeScale::Hour => 0.6,
                TimeScale::Day => 0.4,
            };
            weighted_sum += score * weight;
            total_weight += weight;
        }

        self.combined_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Boost score if multiple scales agree
        let num_triggered = scale_results.iter().filter(|(_, s, _)| *s > 0.5).count();
        if num_triggered >= 2 {
            self.combined_score = (self.combined_score * 1.2).min(1.0);
        }

        self.active_scales = scale_results.iter().map(|(s, _, _)| *s).collect();

        MultiScaleResult {
            combined_score: self.combined_score,
            is_anomaly: self.combined_score > 0.6,
            active_scales: scale_results,
            primary_scale,
            has_seasonality: any_seasonality,
        }
    }

    /// Get statistics for all scales
    pub fn get_stats(&self) -> Vec<(TimeScale, usize, f64, bool)> {
        vec![
            (
                TimeScale::Second,
                self.second_level.get_stats().0,
                self.second_level.get_stats().1,
                self.second_level.get_stats().2,
            ),
            (
                TimeScale::Minute,
                self.minute_level.get_stats().0,
                self.minute_level.get_stats().1,
                self.minute_level.get_stats().2,
            ),
            (
                TimeScale::Hour,
                self.hour_level.get_stats().0,
                self.hour_level.get_stats().1,
                self.hour_level.get_stats().2,
            ),
            (
                TimeScale::Day,
                self.day_level.get_stats().0,
                self.day_level.get_stats().1,
                self.day_level.get_stats().2,
            ),
        ]
    }

    /// Reset all scales
    pub fn reset(&mut self) {
        self.second_level = ScaleDetector::new(TimeScale::Second);
        self.minute_level = ScaleDetector::new(TimeScale::Minute);
        self.hour_level = ScaleDetector::new(TimeScale::Hour);
        self.day_level = ScaleDetector::new(TimeScale::Day);
        self.combined_score = 0.0;
        self.active_scales.clear();
        self.last_timestamp = 0;
        self.sample_count = 0;
    }
}

/// Seasonal decomposition helper
pub struct SeasonalDecomposer {
    /// Observed values
    observations: VecDeque<f64>,
    /// Trend component (smoothed)
    trend: VecDeque<f64>,
    /// Seasonal component
    seasonal: VecDeque<f64>,
    /// Residual (irregular)
    residual: VecDeque<f64>,
    /// Seasonal period
    period: usize,
    /// Whether decomposition is valid
    is_valid: bool,
}

impl SeasonalDecomposer {
    pub fn new(period: usize) -> Self {
        let p = period.max(2);
        Self {
            observations: VecDeque::with_capacity(p * 3),
            trend: VecDeque::with_capacity(p * 3),
            seasonal: VecDeque::with_capacity(p),
            residual: VecDeque::with_capacity(p * 3),
            period: p,
            is_valid: false,
        }
    }

    /// Add observation and update decomposition
    pub fn update(&mut self, value: f64) {
        self.observations.push_back(value);
        if self.observations.len() > self.period * 3 {
            self.observations.pop_front();
        }

        // Decompose when we have enough data
        if self.observations.len() >= self.period * 2 {
            self.decompose();
        }
    }

    /// Simple additive decomposition: Y = T + S + R
    fn decompose(&mut self) {
        let data: Vec<f64> = self.observations.iter().copied().collect();
        let n = data.len();

        // 1. Estimate trend using moving average
        self.trend.clear();
        let half_period = self.period / 2;
        for i in 0..n {
            let start = i.saturating_sub(half_period);
            let end = (i + half_period + 1).min(n);
            if end > start {
                let avg = data[start..end].iter().sum::<f64>() / (end - start) as f64;
                self.trend.push_back(avg);
            }
        }

        // 2. Detrend: Y - T
        let detrended: Vec<f64> = data
            .iter()
            .enumerate()
            .map(|(i, &y)| {
                let t = self.trend.get(i).unwrap_or(&y);
                y - t
            })
            .collect();

        // 3. Estimate seasonal component (average by position in period)
        self.seasonal = (0..self.period)
            .map(|pos| {
                let values: Vec<f64> = detrended
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % self.period == pos)
                    .map(|(_, &v)| v)
                    .collect();
                if !values.is_empty() {
                    values.iter().sum::<f64>() / values.len() as f64
                } else {
                    0.0
                }
            })
            .collect();

        // 4. Compute residual
        self.residual = detrended
            .iter()
            .enumerate()
            .map(|(i, &dt)| {
                let s = self.seasonal.get(i % self.period).unwrap_or(&0.0);
                dt - s
            })
            .collect();

        self.is_valid = true;
    }

    /// Get most recent residual (anomaly indicator)
    pub fn get_residual(&self) -> Option<f64> {
        self.residual.back().copied()
    }

    /// Check if decomposition is valid
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Detect anomaly in residual
    pub fn is_anomalous(&self, threshold_sigma: f64) -> bool {
        if !self.is_valid || self.residual.len() < 10 {
            return false;
        }

        let residual = match self.get_residual() {
            Some(r) => r,
            None => return false,
        };

        // Calculate mean and std of residuals
        let mean = self.residual.iter().sum::<f64>() / self.residual.len() as f64;
        let variance = self
            .residual
            .iter()
            .map(|&r| (r - mean).powi(2))
            .sum::<f64>()
            / self.residual.len() as f64;
        let std = variance.sqrt();

        (residual - mean).abs() > threshold_sigma * std
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_detector_creation() {
        let detector = ScaleDetector::new(TimeScale::Second);
        let (buf_size, _, _) = detector.get_stats();
        assert_eq!(buf_size, 0);
    }

    #[test]
    fn test_multi_scale_basic() {
        let mut detector = MultiScaleDetector::new();
        let mut ts = 0u64;

        // Feed data
        for i in 0..100 {
            let result = detector.update(100.0 + (i % 10) as f64, ts);
            ts += 100_000_000; // 100ms increments

            if i > 10 {
                assert!(result.combined_score >= 0.0);
                assert!(result.combined_score <= 1.0);
            }
        }
    }

    #[test]
    fn test_multi_scale_detects_anomaly() {
        let mut detector = MultiScaleDetector::new();
        let mut ts = 0u64;

        // Normal pattern
        for _ in 0..200 {
            detector.update(100.0, ts);
            ts += 1_000_000_000; // 1 second
        }

        // Anomaly
        let result = detector.update(500.0, ts);

        assert!(result.combined_score > 0.0, "Should detect anomaly");
    }

    #[test]
    fn test_seasonal_decomposer() {
        let mut decomposer = SeasonalDecomposer::new(7);

        // Add seasonal data
        for i in 0..30 {
            let value = 100.0 + (i % 7) as f64 * 10.0; // Weekly pattern
            decomposer.update(value);
        }

        assert!(decomposer.is_valid(), "Should have valid decomposition");

        let residual = decomposer.get_residual();
        assert!(residual.is_some());
    }

    #[test]
    fn test_seasonal_anomaly_detection() {
        let mut decomposer = SeasonalDecomposer::new(5);

        // Normal seasonal pattern
        for i in 0..25 {
            decomposer.update((i % 5) as f64 * 10.0);
        }

        assert!(
            !decomposer.is_anomalous(2.0),
            "Normal pattern should not be anomalous"
        );

        // Anomaly (breaks pattern)
        decomposer.update(1000.0);

        assert!(
            decomposer.is_anomalous(2.0),
            "Should detect anomalous value"
        );
    }

    #[test]
    fn test_scale_specific_results() {
        let mut detector = MultiScaleDetector::new();
        let mut ts = 0u64;

        // Feed data for multiple seconds
        for i in 0..10 {
            let result = detector.update(100.0, ts);
            ts += 1_000_000_000; // 1 second

            if i >= 5 {
                assert!(
                    !result.active_scales.is_empty(),
                    "Should have active scales"
                );
            }
        }
    }
}
