//! Adaptive Ensemble with Dynamic Weight Learning
//!
//! This module implements an ensemble that dynamically adjusts detector weights
//! based on their recent performance using a multi-armed bandit approach.
//!
//! Key features:
//! - Thompson Sampling for weight optimization
//! - Precision/Recall tracking per detector
//! - Automatic weight adaptation based on feedback
//! - Confidence-based ensemble voting
//! - P² algorithm for O(1) percentile estimation
//!
//! Reference: Contextual Bandits for Online Learning

use crate::signal::NUM_DETECTORS;
use rand_distr::Distribution;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct P2QuantileEstimator {
    quantile: f64,
    positions: [f64; 5],
    heights: [f64; 5],
    desired_positions: [f64; 5],
    count: u64,
    initialized: bool,
    init_values: Vec<f64>,
}

impl P2QuantileEstimator {
    fn new(quantile: f64) -> Self {
        Self {
            quantile: quantile.clamp(0.0, 1.0),
            positions: [0.0; 5],
            heights: [f64::NAN; 5],
            desired_positions: [0.0; 5],
            count: 0,
            initialized: false,
            init_values: Vec::with_capacity(5),
        }
    }

    fn update(&mut self, value: f64) {
        self.count += 1;

        if !self.initialized {
            self.init_values.push(value);
            if self.init_values.len() >= 5 {
                self.init_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for i in 0..5 {
                    self.heights[i] = self.init_values[i];
                    self.positions[i] = i as f64;
                }
                self.update_desired_positions();
                self.initialized = true;
            }
            return;
        }

        let _k = if value < self.heights[0] {
            self.heights[0] = value;
            0
        } else if value >= self.heights[4] {
            self.heights[4] = value;
            3
        } else {
            let mut found = 3;
            for i in 0..4 {
                if value < self.heights[i + 1] {
                    found = i;
                    break;
                }
            }
            for i in (found + 1)..5 {
                self.positions[i] += 1.0;
            }
            found
        };

        self.update_desired_positions();
        self.adjust_positions();
    }

    fn update_desired_positions(&mut self) {
        let n = self.count as f64;
        let q = self.quantile;
        self.desired_positions[0] = 0.0;
        self.desired_positions[1] = 2.0 * n * q;
        self.desired_positions[2] = n * q;
        self.desired_positions[3] = n * (1.0 + q);
        self.desired_positions[4] = n;
    }

    fn adjust_positions(&mut self) {
        for i in 1..4 {
            let d = self.desired_positions[i] - self.positions[i];
            if (d >= 1.0 && self.positions[i + 1] - self.positions[i] > 1.0)
                || (d <= -1.0 && self.positions[i - 1] - self.positions[i] < -1.0)
            {
                let sign = d.signum();
                let new_height = self.parabolic(i, sign);
                if self.heights[i - 1] < new_height && new_height < self.heights[i + 1] {
                    self.heights[i] = new_height;
                } else {
                    self.heights[i] = self.linear(i, sign);
                }
                self.positions[i] += sign;
            }
        }
    }

    fn parabolic(&self, i: usize, d: f64) -> f64 {
        let h = &self.heights;
        let p = &self.positions;

        let numerator = (p[i] - p[i - 1] + d) * (h[i + 1] - h[i]) / (p[i + 1] - p[i])
            + (p[i + 1] - p[i] - d) * (h[i] - h[i - 1]) / (p[i] - p[i - 1]);
        let denominator = p[i + 1] - p[i - 1];

        if denominator.abs() < 1e-10 {
            return h[i];
        }

        h[i] + d / denominator * numerator
    }

    fn linear(&self, i: usize, d: f64) -> f64 {
        let h = &self.heights;
        let p = &self.positions;

        if d > 0.0 {
            h[i] + (h[i + 1] - h[i]) / (p[i + 1] - p[i])
        } else {
            h[i] + (h[i - 1] - h[i]) / (p[i - 1] - p[i])
        }
    }

    fn get_quantile(&self) -> f64 {
        if !self.initialized {
            if self.init_values.is_empty() {
                return 0.5;
            }
            let mut sorted = self.init_values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let idx = ((sorted.len() - 1) as f64 * self.quantile) as usize;
            sorted[idx.min(sorted.len() - 1)]
        } else {
            self.heights[2]
        }
    }
}

/// Performance metrics for a single detector
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct DetectorPerformance {
    /// True positives (correctly detected anomalies)
    tp: u64,
    /// False positives (false alarms)
    fp: u64,
    /// True negatives (correctly identified normal)
    tn: u64,
    /// False negatives (missed anomalies)
    fn_: u64,
    /// Recent detection scores for variance estimation
    recent_scores: VecDeque<f64>,
    /// Recent window size for statistics
    window_size: usize,
}

impl DetectorPerformance {
    fn new(window_size: usize) -> Self {
        Self {
            tp: 0,
            fp: 0,
            tn: 0,
            fn_: 0,
            recent_scores: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    /// Update with detection result and ground truth
    fn update(&mut self, detected: bool, is_actual_anomaly: bool, score: f64) {
        // Update confusion matrix
        match (detected, is_actual_anomaly) {
            (true, true) => self.tp += 1,
            (true, false) => self.fp += 1,
            (false, false) => self.tn += 1,
            (false, true) => self.fn_ += 1,
        }

        // Track recent scores
        self.recent_scores.push_back(score);
        if self.recent_scores.len() > self.window_size {
            self.recent_scores.pop_front();
        }
    }

    /// Calculate precision
    fn precision(&self) -> f64 {
        let denom = self.tp + self.fp;
        if denom == 0 {
            0.5 // Prior
        } else {
            self.tp as f64 / denom as f64
        }
    }

    /// Calculate recall
    fn recall(&self) -> f64 {
        let denom = self.tp + self.fn_;
        if denom == 0 {
            0.5 // Prior
        } else {
            self.tp as f64 / denom as f64
        }
    }

    /// Calculate F1 score
    fn f1_score(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }

    /// Calculate recent score variance (confidence proxy)
    #[allow(dead_code)]
    fn score_variance(&self) -> f64 {
        if self.recent_scores.len() < 2 {
            return 1.0; // High variance = low confidence initially
        }

        let mean = self.recent_scores.iter().sum::<f64>() / self.recent_scores.len() as f64;
        let variance = self
            .recent_scores
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / self.recent_scores.len() as f64;

        variance.max(0.001) // Minimum variance for numerical stability
    }
}

/// Thompson Sampling bandit for weight optimization
#[derive(Serialize, Deserialize, Clone)]
pub struct ThompsonBandit {
    /// Number of detectors (arms)
    num_arms: usize,
    /// Alpha parameters for Beta distributions (successes + 1)
    alphas: Vec<f64>,
    /// Beta parameters for Beta distributions (failures + 1)
    betas: Vec<f64>,
    /// Decay factor for non-stationary environments (0.0 to 1.0)
    decay_factor: f64,
}

impl ThompsonBandit {
    pub fn new(num_arms: usize) -> Self {
        let n = num_arms.max(1);
        // Initialize with uniform Beta(1, 1) priors
        Self {
            num_arms: n,
            alphas: vec![1.0; n],
            betas: vec![1.0; n],
            decay_factor: 0.98, // Slowly forget old history
        }
    }

    /// Sample from Beta distributions to get weights
    pub fn sample_weights(&self) -> Vec<f64> {
        let mut weights = Vec::with_capacity(self.num_arms);

        for i in 0..self.num_arms {
            // Clamp alpha and beta to valid ranges for Beta distribution
            let alpha = self.alphas[i].max(0.01);
            let beta = self.betas[i].max(0.01);
            let beta_dist = rand_distr::Beta::new(alpha, beta).unwrap();
            let sample: f64 = beta_dist.sample(&mut rand::rng());
            weights.push(sample);
        }

        // Normalize to sum to 1
        let sum: f64 = weights.iter().sum();
        if sum > 0.0 {
            weights.iter_mut().for_each(|w| *w /= sum);
        }

        weights
    }

    /// Update with feedback (success = 1, failure = 0)
    pub fn update(&mut self, arm: usize, success: bool) {
        if arm >= self.num_arms {
            return;
        }

        // Apply decay to prevent infinite confidence accumulation
        // This ensures the bandit remains adaptive to concept drift
        self.alphas[arm] *= self.decay_factor;
        self.betas[arm] *= self.decay_factor;

        // Ensure we don't drop below priors
        self.alphas[arm] = self.alphas[arm].max(1.0);
        self.betas[arm] = self.betas[arm].max(1.0);

        if success {
            self.alphas[arm] += 1.0;
        } else {
            self.betas[arm] += 1.0;
        }
    }

    /// Get expected values (mean of Beta distributions)
    pub fn expected_weights(&self) -> Vec<f64> {
        let mut weights = Vec::with_capacity(self.num_arms);

        for i in 0..self.num_arms {
            // Beta distribution mean = alpha / (alpha + beta)
            let mean = self.alphas[i] / (self.alphas[i] + self.betas[i]);
            weights.push(mean);
        }

        // Normalize
        let sum: f64 = weights.iter().sum();
        if sum > 0.0 {
            weights.iter_mut().for_each(|w| *w /= sum);
        }

        weights
    }

    /// Get raw alpha/beta values for debugging
    pub fn get_params(&self) -> Vec<(f64, f64)> {
        self.alphas
            .iter()
            .cloned()
            .zip(self.betas.iter().cloned())
            .collect()
    }

    /// Restore alpha/beta parameters from checkpointed state.
    pub fn set_params(&mut self, alphas: &[f64], betas: &[f64]) -> Result<(), &'static str> {
        if alphas.len() != self.num_arms || betas.len() != self.num_arms {
            return Err("invalid bandit parameter length");
        }
        self.alphas.clone_from_slice(alphas);
        self.betas.clone_from_slice(betas);
        Ok(())
    }
}

/// Adaptive Ensemble that learns optimal detector weights
#[derive(Serialize, Deserialize, Clone)]
pub struct AdaptiveEnsemble {
    /// Number of detectors in ensemble
    num_detectors: usize,
    /// Performance tracking per detector
    performance: Vec<DetectorPerformance>,
    /// Thompson sampling bandit for weight learning
    bandit: ThompsonBandit,
    /// Current weights (updated periodically)
    current_weights: [f64; NUM_DETECTORS],
    /// Whether to use Thompson sampling (exploration) or expected values (exploitation)
    exploration_rate: f64,
    /// Update counter
    update_count: u64,
    /// Weight update interval
    update_interval: usize,
    /// Detector names for reference
    detector_names: Vec<String>,
    /// P² estimator for O(1) percentile calculation
    #[serde(skip)]
    p2_estimator: P2QuantileEstimator,
    /// Adaptive threshold
    adaptive_threshold: f64,
}

/// Detection result from individual detector
#[derive(Clone, Copy, Debug, Default)]
pub struct DetectorOutput {
    pub detector_id: usize,
    pub score: f64,
    pub confidence: f64,
    pub signal_type: u8,
}

impl AdaptiveEnsemble {
    /// Create new adaptive ensemble
    ///
    /// # Arguments
    /// * `detector_names` - Names of detectors in ensemble
    /// * `exploration_rate` - Probability of exploration vs exploitation (0.0-1.0)
    /// * `update_interval` - How often to update weights (in samples)
    pub fn new(detector_names: Vec<String>, exploration_rate: f64, update_interval: usize) -> Self {
        let n = detector_names.len().clamp(1, NUM_DETECTORS);
        let exploration = exploration_rate.clamp(0.0, 1.0);
        let mut current_weights = [0.0; NUM_DETECTORS];
        let uniform = 1.0 / n as f64;
        for w in current_weights.iter_mut().take(n) {
            *w = uniform;
        }

        Self {
            num_detectors: n,
            performance: (0..n).map(|_| DetectorPerformance::new(100)).collect(),
            bandit: ThompsonBandit::new(n),
            current_weights,
            exploration_rate: exploration,
            update_count: 0,
            update_interval: update_interval.max(10),
            detector_names,
            p2_estimator: P2QuantileEstimator::new(0.95),
            adaptive_threshold: 0.5,
        }
    }

    /// Create with default settings
    pub fn default_ensemble(detector_names: Vec<String>) -> Self {
        Self::new(detector_names, 0.1, 100)
    }

    /// Combine detector outputs into ensemble score
    pub fn combine(&mut self, outputs: &[DetectorOutput]) -> (f64, f64) {
        if outputs.is_empty() {
            return (0.0, 0.0);
        }

        self.update_count += 1;

        // Calculate weighted ensemble score
        let mut weighted_score = 0.0;
        let mut total_weight = 0.0;
        let mut triggered = 0usize;

        for output in outputs {
            if output.detector_id < self.num_detectors {
                let weight = self.current_weights[output.detector_id];
                let weighted = output.score * weight * output.confidence;
                weighted_score += weighted;
                total_weight += weight * output.confidence;
                if output.score > 0.5 {
                    triggered += 1;
                }
            }
        }

        let ensemble_score = if total_weight > 0.0 {
            weighted_score / total_weight
        } else {
            0.0
        };

        // Calculate ensemble confidence
        let confidence = self.calculate_confidence(outputs, triggered);

        // Update score history and adaptive threshold
        self.update_threshold(ensemble_score);

        (ensemble_score, confidence)
    }

    /// Update weights based on ground truth feedback
    ///
    /// Call this when you receive confirmation (from Tier-2 or human review)
    /// about whether an actual anomaly occurred.
    pub fn update_with_feedback(
        &mut self,
        outputs: &[DetectorOutput],
        _ensemble_detected: bool,
        was_actual_anomaly: bool,
    ) {
        // Update individual detector performance AND bandit weights
        // We treat each detector as an arm that we want to learn the reliability of
        for output in outputs {
            if output.detector_id < self.num_detectors {
                let detected = output.score > 0.5; // Assuming 0.5 threshold

                // 1. Update Precision/Recall stats
                self.performance[output.detector_id].update(
                    detected,
                    was_actual_anomaly,
                    output.score,
                );

                // 2. Update Bandit (Weight Learning)
                // If it was an anomaly, did this detector find it? (True Positive)
                // If it was normal, did this detector ignore it? (True Negative)
                let success = if was_actual_anomaly {
                    detected // Reward if it detected the anomaly
                } else {
                    !detected // Reward if it correctly stayed silent
                };

                self.bandit.update(output.detector_id, success);
            }
        }

        // Update weights periodically
        if self.update_count % self.update_interval as u64 == 0 {
            self.update_weights();
        }
    }

    /// Update current weights based on performance and bandit
    fn update_weights(&mut self) {
        // Get Thompson sampling weights
        let thompson_weights = if rand::random::<f64>() < self.exploration_rate {
            self.bandit.sample_weights() // Explore
        } else {
            self.bandit.expected_weights() // Exploit
        };

        // Get performance-based weights (F1 scores)
        let f1_weights: Vec<f64> = self.performance.iter().map(|p| p.f1_score()).collect();

        // Normalize F1 weights
        let f1_sum: f64 = f1_weights.iter().sum();
        let normalized_f1: Vec<f64> = if f1_sum > 0.0 {
            f1_weights.iter().map(|&w| w / f1_sum).collect()
        } else {
            vec![1.0 / self.num_detectors as f64; self.num_detectors]
        };

        // Combine Thompson and F1 weights (equal blend)
        for i in 0..self.num_detectors {
            if i < thompson_weights.len() && i < normalized_f1.len() {
                self.current_weights[i] = 0.5 * thompson_weights[i] + 0.5 * normalized_f1[i];
            }
        }

        // Renormalize
        let sum: f64 = self.current_weights.iter().take(self.num_detectors).sum();
        if sum > 0.0 {
            self.current_weights
                .iter_mut()
                .take(self.num_detectors)
                .for_each(|w| *w /= sum);
        }
        self.current_weights
            .iter_mut()
            .skip(self.num_detectors)
            .for_each(|w| *w = 0.0);
    }

    /// Calculate ensemble confidence
    fn calculate_confidence(&self, outputs: &[DetectorOutput], triggered: usize) -> f64 {
        if outputs.is_empty() {
            return 0.0;
        }

        // Agreement between detectors
        let agreement = triggered as f64 / self.num_detectors as f64;

        // Weighted average of individual confidences
        let mut total_confidence = 0.0;
        let mut total_weight = 0.0;

        for output in outputs {
            let weight = self.current_weights[output.detector_id];
            total_confidence += output.confidence * weight;
            total_weight += weight;
        }

        let avg_confidence = if total_weight > 0.0 {
            total_confidence / total_weight
        } else {
            0.5
        };

        // Combine agreement and confidence
        0.6 * agreement + 0.4 * avg_confidence
    }

    /// Update adaptive threshold based on score distribution
    /// Uses P² algorithm for O(1) percentile estimation
    fn update_threshold(&mut self, score: f64) {
        self.p2_estimator.update(score);

        if self.p2_estimator.count >= 100 {
            self.adaptive_threshold = self.p2_estimator.get_quantile().max(0.5);
        }
    }

    /// Get current weights
    pub fn get_weights(&self) -> Vec<(String, f64)> {
        self.detector_names
            .iter()
            .take(self.num_detectors)
            .cloned()
            .zip(
                self.current_weights
                    .iter()
                    .take(self.num_detectors)
                    .cloned(),
            )
            .collect()
    }

    /// Current normalized detector weights.
    pub fn current_weights(&self) -> &[f64] {
        &self.current_weights[..self.num_detectors]
    }

    /// Restore full adaptive state from a checkpoint.
    pub fn restore_state(
        &mut self,
        weights: &[f64],
        alphas: &[f64],
        betas: &[f64],
        total_samples: u64,
    ) -> Result<(), &'static str> {
        if weights.len() != self.num_detectors {
            return Err("invalid weight length");
        }
        self.current_weights[..self.num_detectors].copy_from_slice(weights);
        self.current_weights
            .iter_mut()
            .skip(self.num_detectors)
            .for_each(|w| *w = 0.0);
        let sum: f64 = self.current_weights.iter().take(self.num_detectors).sum();
        if sum <= 0.0 {
            return Err("invalid weight sum");
        }
        self.current_weights
            .iter_mut()
            .take(self.num_detectors)
            .for_each(|w| *w /= sum);
        self.bandit.set_params(alphas, betas)?;
        self.update_count = total_samples;
        Ok(())
    }

    /// Export bandit alpha/beta arrays.
    pub fn bandit_params(&self) -> (Vec<f64>, Vec<f64>) {
        let params = self.bandit.get_params();
        let mut alphas = Vec::with_capacity(params.len());
        let mut betas = Vec::with_capacity(params.len());
        for (a, b) in params {
            alphas.push(a);
            betas.push(b);
        }
        (alphas, betas)
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> Vec<(String, f64, f64, f64)> {
        self.performance
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let name = self.detector_names.get(i).cloned().unwrap_or_default();
                (name, p.precision(), p.recall(), p.f1_score())
            })
            .collect()
    }

    /// Check if score exceeds adaptive threshold
    pub fn is_anomaly(&self, score: f64) -> bool {
        score > self.adaptive_threshold
    }

    /// Get adaptive threshold
    pub fn get_threshold(&self) -> f64 {
        self.adaptive_threshold
    }

    /// Reset all learning
    pub fn reset(&mut self) {
        self.performance = (0..self.num_detectors)
            .map(|_| DetectorPerformance::new(100))
            .collect();
        self.bandit = ThompsonBandit::new(self.num_detectors);
        self.current_weights = [0.0; NUM_DETECTORS];
        let uniform = 1.0 / self.num_detectors as f64;
        for w in self.current_weights.iter_mut().take(self.num_detectors) {
            *w = uniform;
        }
        self.update_count = 0;
        self.p2_estimator = P2QuantileEstimator::new(0.95);
        self.adaptive_threshold = 0.5;
    }
}

/// UCB1 Bandit (alternative to Thompson Sampling)
#[derive(Serialize, Deserialize, Clone)]
pub struct UCBBandit {
    /// Number of arms
    num_arms: usize,
    /// Total reward per arm
    rewards: Vec<f64>,
    /// Number of pulls per arm
    counts: Vec<u64>,
    /// Total pulls
    total_count: u64,
    /// Exploration parameter
    c: f64,
}

impl UCBBandit {
    pub fn new(num_arms: usize, c: f64) -> Self {
        let n = num_arms.max(1);
        Self {
            num_arms: n,
            rewards: vec![0.0; n],
            counts: vec![0; n],
            total_count: 0,
            c: c.max(0.1),
        }
    }

    /// Select arm using UCB1 formula
    pub fn select_arm(&self) -> usize {
        // Try each arm at least once
        for (i, &count) in self.counts.iter().enumerate() {
            if count == 0 {
                return i;
            }
        }

        // UCB1 formula: mean_reward + c * sqrt(2 * ln(total) / count)
        let mut best_arm = 0;
        let mut best_value = 0.0;

        for i in 0..self.num_arms {
            let mean_reward = self.rewards[i] / self.counts[i] as f64;
            let confidence = (2.0 * (self.total_count as f64).ln() / self.counts[i] as f64).sqrt();
            let ucb_value = mean_reward + self.c * confidence;

            if ucb_value > best_value {
                best_value = ucb_value;
                best_arm = i;
            }
        }

        best_arm
    }

    /// Update arm with reward
    pub fn update(&mut self, arm: usize, reward: f64) {
        if arm >= self.num_arms {
            return;
        }
        self.rewards[arm] += reward.clamp(0.0, 1.0);
        self.counts[arm] += 1;
        self.total_count += 1;
    }

    /// Get current estimated values
    pub fn get_values(&self) -> Vec<f64> {
        self.rewards
            .iter()
            .enumerate()
            .map(|(i, &reward)| {
                if self.counts[i] == 0 {
                    0.5
                } else {
                    reward / self.counts[i] as f64
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_ensemble_creation() {
        let names = vec![
            "Volume".to_string(),
            "Distribution".to_string(),
            "Cardinality".to_string(),
        ];
        let ensemble = AdaptiveEnsemble::new(names.clone(), 0.1, 100);

        let weights = ensemble.get_weights();
        assert_eq!(weights.len(), 3);

        // Initial weights should be uniform
        for (_, w) in &weights {
            assert!(
                (w - 0.333).abs() < 0.01,
                "Initial weights should be uniform"
            );
        }
    }

    #[test]
    fn test_ensemble_combine() {
        let names = vec!["A".to_string(), "B".to_string()];
        let mut ensemble = AdaptiveEnsemble::new(names, 0.0, 10); // No exploration

        let outputs = vec![
            DetectorOutput {
                detector_id: 0,
                score: 0.8,
                confidence: 0.9,
                signal_type: 1,
            },
            DetectorOutput {
                detector_id: 1,
                score: 0.3,
                confidence: 0.7,
                signal_type: 2,
            },
        ];

        let (score, confidence) = ensemble.combine(&outputs);

        assert!(score > 0.0, "Should have positive score");
        assert!(
            confidence >= 0.0 && confidence <= 1.0,
            "Confidence should be normalized"
        );
    }

    #[test]
    fn test_feedback_updates() {
        let names = vec!["A".to_string(), "B".to_string()];
        let mut ensemble = AdaptiveEnsemble::new(names, 0.0, 5);

        let outputs = vec![
            DetectorOutput {
                detector_id: 0,
                score: 0.9,
                confidence: 0.9,
                signal_type: 1,
            },
            DetectorOutput {
                detector_id: 1,
                score: 0.1,
                confidence: 0.7,
                signal_type: 2,
            },
        ];

        // Simulate correct detection
        ensemble.update_with_feedback(&outputs, true, true);

        let stats = ensemble.get_performance_stats();
        assert_eq!(stats.len(), 2);

        // Detector A should have TP=1
        assert_eq!(stats[0].1, 1.0, "Detector A should have 100% precision"); // Precision
        assert_eq!(stats[0].2, 1.0, "Detector A should have 100% recall"); // Recall
    }

    #[test]
    fn test_ucb_bandit() {
        let mut bandit = UCBBandit::new(3, 1.0);

        // Simulate some rewards
        bandit.update(0, 1.0);
        bandit.update(0, 1.0);
        bandit.update(1, 0.5);
        bandit.update(2, 0.0);

        let arm = bandit.select_arm();
        // Arm 0 should be preferred (highest reward)
        assert_eq!(arm, 0, "UCB should select arm with highest reward");

        let values = bandit.get_values();
        assert!(values[0] > values[1], "Arm 0 should have higher value");
    }

    #[test]
    fn test_thompson_sampling() {
        let mut bandit = ThompsonBandit::new(3);

        // Update with clear successes for arm 0
        bandit.update(0, true);
        bandit.update(0, true);
        bandit.update(0, true);
        bandit.update(0, true);
        bandit.update(0, true);

        // Arm 1 gets fewer successes
        bandit.update(1, true);
        bandit.update(1, false);

        // Arm 2 gets failures
        bandit.update(2, false);
        bandit.update(2, false);

        let weights = bandit.expected_weights();
        assert_eq!(weights.len(), 3);

        // Arm 0 should have highest weight (5 successes, 0 failures)
        // Expected value = alpha/(alpha+beta) = 6/7 = 0.857
        // Arm 1 = 2/(2+2) = 0.5
        // Arm 2 = 1/(1+3) = 0.25
        assert!(
            weights[0] > weights[1],
            "Arm 0 should be preferred over arm 1: {} vs {}",
            weights[0],
            weights[1]
        );
        assert!(
            weights[0] > weights[2],
            "Arm 0 should be preferred over arm 2: {} vs {}",
            weights[0],
            weights[2]
        );
    }

    #[test]
    fn test_detector_performance() {
        let mut perf = DetectorPerformance::new(10);

        // Simulate detections
        perf.update(true, true, 0.9); // TP
        perf.update(true, true, 0.85); // TP
        perf.update(true, false, 0.6); // FP
        perf.update(false, true, 0.0); // FN
        perf.update(false, false, 0.0); // TN

        assert_eq!(perf.precision(), 2.0 / 3.0, "Precision should be 2/3");
        assert_eq!(perf.recall(), 2.0 / 3.0, "Recall should be 2/3");
    }
}
