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
//!
//! Reference: Contextual Bandits for Online Learning

use rand_distr::Distribution;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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
struct ThompsonBandit {
    /// Number of detectors (arms)
    num_arms: usize,
    /// Alpha parameters for Beta distributions (successes)
    alphas: Vec<f64>,
    /// Beta parameters for Beta distributions (failures)
    betas: Vec<f64>,
    /// Discount factor for old observations
    gamma: f64,
}

impl ThompsonBandit {
    fn new(num_arms: usize) -> Self {
        let n = num_arms.max(1);
        // Initialize with uniform Beta(1, 1) priors
        Self {
            num_arms: n,
            alphas: vec![1.0; n],
            betas: vec![1.0; n],
            gamma: 0.99, // Discount old observations
        }
    }

    /// Sample from Beta distributions to get weights
    fn sample_weights(&self) -> Vec<f64> {
        let mut weights = Vec::with_capacity(self.num_arms);

        for i in 0..self.num_arms {
            let beta_dist = rand_distr::Beta::new(self.alphas[i], self.betas[i]).unwrap();
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
    fn update(&mut self, arm: usize, success: bool) {
        if arm >= self.num_arms {
            return;
        }

        // Apply discount factor to old observations
        for i in 0..self.num_arms {
            self.alphas[i] = 1.0 + self.gamma * (self.alphas[i] - 1.0);
            self.betas[i] = 1.0 + self.gamma * (self.betas[i] - 1.0);
        }

        // Update selected arm
        if success {
            self.alphas[arm] += 1.0;
        } else {
            self.betas[arm] += 1.0;
        }
    }

    /// Get expected values (mean of Beta distributions)
    fn expected_weights(&self) -> Vec<f64> {
        let mut weights = Vec::with_capacity(self.num_arms);

        for i in 0..self.num_arms {
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
    current_weights: Vec<f64>,
    /// Whether to use Thompson sampling (exploration) or expected values (exploitation)
    exploration_rate: f64,
    /// Update counter
    update_count: u64,
    /// Weight update interval
    update_interval: usize,
    /// Detector names for reference
    detector_names: Vec<String>,
    /// Recent ensemble scores for adaptive threshold
    score_history: VecDeque<f64>,
    /// History window size
    history_window: usize,
    /// Adaptive threshold
    adaptive_threshold: f64,
}

/// Detection result from individual detector
#[derive(Clone, Debug)]
pub struct DetectorOutput {
    pub detector_id: usize,
    pub detector_name: String,
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
        let n = detector_names.len().max(1);
        let exploration = exploration_rate.clamp(0.0, 1.0);

        Self {
            num_detectors: n,
            performance: (0..n).map(|_| DetectorPerformance::new(100)).collect(),
            bandit: ThompsonBandit::new(n),
            current_weights: vec![1.0 / n as f64; n],
            exploration_rate: exploration,
            update_count: 0,
            update_interval: update_interval.max(10),
            detector_names,
            score_history: VecDeque::with_capacity(1000),
            history_window: 1000,
            adaptive_threshold: 0.5,
        }
    }

    /// Create with default settings
    pub fn default_ensemble(detector_names: Vec<String>) -> Self {
        Self::new(detector_names, 0.1, 100)
    }

    /// Combine detector outputs into ensemble score
    pub fn combine(&mut self, outputs: &[DetectorOutput]) -> (f64, f64, Vec<f64>) {
        if outputs.is_empty() {
            return (0.0, 0.0, vec![]);
        }

        self.update_count += 1;

        // Calculate weighted ensemble score
        let mut weighted_score = 0.0;
        let mut total_weight = 0.0;
        let mut individual_scores = vec![0.0; self.num_detectors];

        for output in outputs {
            if output.detector_id < self.num_detectors {
                let weight = self.current_weights[output.detector_id];
                let weighted = output.score * weight * output.confidence;
                weighted_score += weighted;
                total_weight += weight * output.confidence;
                individual_scores[output.detector_id] = output.score;
            }
        }

        let ensemble_score = if total_weight > 0.0 {
            weighted_score / total_weight
        } else {
            0.0
        };

        // Calculate ensemble confidence
        let confidence = self.calculate_confidence(outputs, &individual_scores);

        // Update score history and adaptive threshold
        self.update_threshold(ensemble_score);

        (ensemble_score, confidence, individual_scores)
    }

    /// Update weights based on ground truth feedback
    ///
    /// Call this when you receive confirmation (from Tier-2 or human review)
    /// about whether an actual anomaly occurred.
    pub fn update_with_feedback(
        &mut self,
        outputs: &[DetectorOutput],
        ensemble_detected: bool,
        was_actual_anomaly: bool,
    ) {
        // Update individual detector performance
        for output in outputs {
            if output.detector_id < self.num_detectors {
                let detected = output.score > 0.5; // Assuming 0.5 threshold
                self.performance[output.detector_id].update(
                    detected,
                    was_actual_anomaly,
                    output.score,
                );
            }
        }

        // Update bandit with ensemble-level feedback
        // Success = correct detection (detected and was anomaly, or not detected and was normal)
        let success = ensemble_detected == was_actual_anomaly;

        // Find the detector with highest contribution
        let best_detector = outputs
            .iter()
            .max_by(|a, b| {
                let a_score = a.score * self.current_weights[a.detector_id];
                let b_score = b.score * self.current_weights[b.detector_id];
                a_score.partial_cmp(&b_score).unwrap()
            })
            .map(|o| o.detector_id)
            .unwrap_or(0);

        self.bandit.update(best_detector, success);

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
            self.current_weights[i] = 0.5 * thompson_weights[i] + 0.5 * normalized_f1[i];
        }

        // Renormalize
        let sum: f64 = self.current_weights.iter().sum();
        if sum > 0.0 {
            self.current_weights.iter_mut().for_each(|w| *w /= sum);
        }
    }

    /// Calculate ensemble confidence
    fn calculate_confidence(&self, outputs: &[DetectorOutput], individual_scores: &[f64]) -> f64 {
        if outputs.is_empty() {
            return 0.0;
        }

        // Agreement between detectors
        let num_triggered = individual_scores.iter().filter(|&&s| s > 0.5).count();
        let agreement = num_triggered as f64 / self.num_detectors as f64;

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
    fn update_threshold(&mut self, score: f64) {
        self.score_history.push_back(score);
        if self.score_history.len() > self.history_window {
            self.score_history.pop_front();
        }

        if self.score_history.len() >= 100 {
            // Calculate 95th percentile as threshold
            let mut sorted: Vec<f64> = self.score_history.iter().copied().collect();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let idx = (0.95 * (sorted.len() - 1) as f64) as usize;
            self.adaptive_threshold = sorted[idx.min(sorted.len() - 1)].max(0.5);
        }
    }

    /// Get current weights
    pub fn get_weights(&self) -> Vec<(String, f64)> {
        self.detector_names
            .iter()
            .cloned()
            .zip(self.current_weights.iter().cloned())
            .collect()
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
        self.current_weights = vec![1.0 / self.num_detectors as f64; self.num_detectors];
        self.update_count = 0;
        self.score_history.clear();
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
                detector_name: "A".to_string(),
                score: 0.8,
                confidence: 0.9,
                signal_type: 1,
            },
            DetectorOutput {
                detector_id: 1,
                detector_name: "B".to_string(),
                score: 0.3,
                confidence: 0.7,
                signal_type: 2,
            },
        ];

        let (score, confidence, individual) = ensemble.combine(&outputs);

        assert!(score > 0.0, "Should have positive score");
        assert!(
            confidence >= 0.0 && confidence <= 1.0,
            "Confidence should be normalized"
        );
        assert_eq!(individual.len(), 2);
    }

    #[test]
    fn test_feedback_updates() {
        let names = vec!["A".to_string(), "B".to_string()];
        let mut ensemble = AdaptiveEnsemble::new(names, 0.0, 5);

        let outputs = vec![
            DetectorOutput {
                detector_id: 0,
                detector_name: "A".to_string(),
                score: 0.9,
                confidence: 0.9,
                signal_type: 1,
            },
            DetectorOutput {
                detector_id: 1,
                detector_name: "B".to_string(),
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

        // Update with successes and failures
        bandit.update(0, true);
        bandit.update(0, true);
        bandit.update(0, false);
        bandit.update(1, true);
        bandit.update(2, false);

        let weights = bandit.expected_weights();
        assert_eq!(weights.len(), 3);

        // Arm 0 should have highest weight (2 successes, 1 failure)
        assert!(weights[0] > weights[1], "Arm 0 should be preferred");
        assert!(
            weights[0] > weights[2],
            "Arm 0 should be preferred over arm 2"
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
