//! Feedback Loop from Tier-2 to Tier-1
//!
//! This module handles the learning feedback from Tier-2 (Bun) back to Tier-1 (Rust).
//! When Tier-2 confirms or denies an anomaly (via LLM, human review, or correlation),
//! it sends feedback that updates the AdaptiveEnsemble weights via Thompson Sampling.

use crate::signal::NUM_DETECTORS;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Feedback event from Tier-2
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FeedbackEvent {
    /// Entity hash that was evaluated
    pub entity_hash: u64,
    /// Original signal timestamp
    pub signal_timestamp: u64,
    /// Whether Tier-2 confirmed this was a true anomaly
    pub was_true_positive: bool,
    /// Original detector scores (for learning which detectors were right)
    pub detector_scores: [f32; NUM_DETECTORS],
    /// Original ensemble decision
    pub original_decision: bool,
    /// Tier-2 confidence in this feedback (0.0 - 1.0)
    pub feedback_confidence: f32,
    /// Source of feedback
    pub feedback_source: FeedbackSource,
}

/// Source of the feedback
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackSource {
    /// LLM analysis in Tier-2
    LLMAnalysis = 0,
    /// Human operator confirmation
    HumanReview = 1,
    /// Automated correlation (matched known pattern)
    AutoCorrelation = 2,
    /// Timeout (no confirmation received, assume false positive)
    Timeout = 3,
}

impl FeedbackEvent {
    /// Create feedback for a true positive confirmation
    pub fn true_positive(
        entity_hash: u64,
        signal_timestamp: u64,
        detector_scores: [f32; NUM_DETECTORS],
        source: FeedbackSource,
        confidence: f32,
    ) -> Self {
        Self {
            entity_hash,
            signal_timestamp,
            was_true_positive: true,
            detector_scores,
            original_decision: true,
            feedback_confidence: confidence,
            feedback_source: source,
        }
    }

    /// Create feedback for a false positive
    pub fn false_positive(
        entity_hash: u64,
        signal_timestamp: u64,
        detector_scores: [f32; NUM_DETECTORS],
        source: FeedbackSource,
        confidence: f32,
    ) -> Self {
        Self {
            entity_hash,
            signal_timestamp,
            was_true_positive: false,
            detector_scores,
            original_decision: true,
            feedback_confidence: confidence,
            feedback_source: source,
        }
    }

    /// Create feedback for a missed detection (false negative)
    pub fn false_negative(
        entity_hash: u64,
        signal_timestamp: u64,
        detector_scores: [f32; NUM_DETECTORS],
        source: FeedbackSource,
        confidence: f32,
    ) -> Self {
        Self {
            entity_hash,
            signal_timestamp,
            was_true_positive: true, // It WAS an anomaly
            detector_scores,
            original_decision: false, // But we said it wasn't
            feedback_confidence: confidence,
            feedback_source: source,
        }
    }

    /// Calculate which detectors were correct
    pub fn correct_detectors(&self) -> [bool; NUM_DETECTORS] {
        let mut correct = [false; NUM_DETECTORS];
        let threshold = 0.5;

        for (i, &score) in self.detector_scores.iter().enumerate() {
            let detector_fired = score >= threshold;
            // Detector is correct if: (fired AND true_pos) OR (not_fired AND not true_pos)
            correct[i] = detector_fired == self.was_true_positive;
        }

        correct
    }
}

/// Statistics for feedback processing
#[derive(Debug, Default)]
pub struct FeedbackStats {
    pub received: AtomicU64,
    pub processed: AtomicU64,
    pub true_positives: AtomicU64,
    pub false_positives: AtomicU64,
    pub false_negatives: AtomicU64,
    pub dropped: AtomicU64,
}

impl FeedbackStats {
    pub fn record_received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_processed(&self, event: &FeedbackEvent) {
        self.processed.fetch_add(1, Ordering::Relaxed);

        match (event.was_true_positive, event.original_decision) {
            (true, true) => {
                self.true_positives.fetch_add(1, Ordering::Relaxed);
            }
            (false, true) => {
                self.false_positives.fetch_add(1, Ordering::Relaxed);
            }
            (true, false) => {
                self.false_negatives.fetch_add(1, Ordering::Relaxed);
            }
            (false, false) => {} // True negative, not tracked
        }
    }

    pub fn record_dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn precision(&self) -> f64 {
        let tp = self.true_positives.load(Ordering::Relaxed) as f64;
        let fp = self.false_positives.load(Ordering::Relaxed) as f64;
        if tp + fp > 0.0 { tp / (tp + fp) } else { 1.0 }
    }

    pub fn recall(&self) -> f64 {
        let tp = self.true_positives.load(Ordering::Relaxed) as f64;
        let fn_ = self.false_negatives.load(Ordering::Relaxed) as f64;
        if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 1.0 }
    }

    pub fn f1_score(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r > 0.0 {
            2.0 * (p * r) / (p + r)
        } else {
            0.0
        }
    }

    pub fn snapshot(&self) -> FeedbackStatsSnapshot {
        FeedbackStatsSnapshot {
            received: self.received.load(Ordering::Relaxed),
            processed: self.processed.load(Ordering::Relaxed),
            true_positives: self.true_positives.load(Ordering::Relaxed),
            false_positives: self.false_positives.load(Ordering::Relaxed),
            false_negatives: self.false_negatives.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            precision: self.precision(),
            recall: self.recall(),
            f1_score: self.f1_score(),
        }
    }
}

/// Serializable snapshot of feedback stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackStatsSnapshot {
    pub received: u64,
    pub processed: u64,
    pub true_positives: u64,
    pub false_positives: u64,
    pub false_negatives: u64,
    pub dropped: u64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
}

/// Channel for receiving feedback from Tier-2
pub struct FeedbackChannel {
    sender: Sender<FeedbackEvent>,
    receiver: Receiver<FeedbackEvent>,
    stats: FeedbackStats,
}

impl FeedbackChannel {
    /// Create a new feedback channel with specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self {
            sender,
            receiver,
            stats: FeedbackStats::default(),
        }
    }

    /// Get a sender handle (for FFI/external use)
    pub fn sender(&self) -> FeedbackSender {
        FeedbackSender {
            sender: self.sender.clone(),
        }
    }

    /// Get a receiver handle (for engine use)
    pub fn receiver(&self) -> FeedbackReceiver<'_> {
        FeedbackReceiver {
            receiver: self.receiver.clone(),
            stats: &self.stats,
        }
    }

    /// Get current stats
    pub fn stats(&self) -> &FeedbackStats {
        &self.stats
    }

    /// Send feedback (non-blocking)
    pub fn try_send(&self, event: FeedbackEvent) -> Result<(), FeedbackEvent> {
        self.stats.record_received();
        match self.sender.try_send(event) {
            Ok(_) => Ok(()),
            Err(TrySendError::Full(e)) => {
                self.stats.record_dropped();
                Err(e)
            }
            Err(TrySendError::Disconnected(e)) => {
                self.stats.record_dropped();
                Err(e)
            }
        }
    }

    /// Receive all pending feedback (non-blocking batch)
    pub fn drain(&self) -> Vec<FeedbackEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            self.stats.record_processed(&event);
            events.push(event);
        }
        events
    }
}

/// Cloneable sender for external use
#[derive(Clone)]
pub struct FeedbackSender {
    sender: Sender<FeedbackEvent>,
}

impl FeedbackSender {
    pub fn send(&self, event: FeedbackEvent) -> Result<(), FeedbackEvent> {
        self.sender.try_send(event).map_err(|e| e.into_inner())
    }
}

/// Receiver for engine use
pub struct FeedbackReceiver<'a> {
    receiver: Receiver<FeedbackEvent>,
    stats: &'a FeedbackStats,
}

impl<'a> FeedbackReceiver<'a> {
    /// Try to receive one event (non-blocking)
    pub fn try_recv(&self) -> Option<FeedbackEvent> {
        self.receiver.try_recv().ok().map(|e| {
            self.stats.record_processed(&e);
            e
        })
    }

    /// Drain all pending events
    pub fn drain(&self) -> Vec<FeedbackEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            self.stats.record_processed(&event);
            events.push(event);
        }
        events
    }
}

/// Aggregated learning update for AdaptiveEnsemble
#[derive(Debug, Clone)]
pub struct LearningUpdate {
    /// For each detector: (successes, failures)
    pub detector_outcomes: [(u32, u32); NUM_DETECTORS],
    /// Overall true positive count
    pub true_positives: u32,
    /// Overall false positive count
    pub false_positives: u32,
    /// Overall false negative count
    pub false_negatives: u32,
}

impl LearningUpdate {
    /// Create from a batch of feedback events
    pub fn from_batch(events: &[FeedbackEvent]) -> Self {
        let mut outcomes = [(0u32, 0u32); NUM_DETECTORS];
        let mut tp = 0u32;
        let mut fp = 0u32;
        let mut fn_ = 0u32;

        for event in events {
            // Weight by confidence
            let weight = (event.feedback_confidence * 10.0).max(1.0) as u32;

            match (event.was_true_positive, event.original_decision) {
                (true, true) => tp += weight,
                (false, true) => fp += weight,
                (true, false) => fn_ += weight,
                (false, false) => {}
            }

            let correct = event.correct_detectors();
            for (i, &is_correct) in correct.iter().enumerate() {
                if is_correct {
                    outcomes[i].0 += weight; // success
                } else {
                    outcomes[i].1 += weight; // failure
                }
            }
        }

        Self {
            detector_outcomes: outcomes,
            true_positives: tp,
            false_positives: fp,
            false_negatives: fn_,
        }
    }

    /// Check if this update has meaningful data
    pub fn is_significant(&self) -> bool {
        self.true_positives + self.false_positives + self.false_negatives > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_channel() {
        let channel = FeedbackChannel::new(100);

        let event = FeedbackEvent::true_positive(
            12345,
            1000000,
            [0.8, 0.6, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            FeedbackSource::LLMAnalysis,
            0.95,
        );

        assert!(channel.try_send(event).is_ok());

        let events = channel.drain();
        assert_eq!(events.len(), 1);
        assert!(events[0].was_true_positive);

        let stats = channel.stats().snapshot();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.processed, 1);
        assert_eq!(stats.true_positives, 1);
    }

    #[test]
    fn test_correct_detectors() {
        let event = FeedbackEvent::true_positive(
            12345,
            1000000,
            [0.8, 0.6, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            FeedbackSource::HumanReview,
            1.0,
        );

        let correct = event.correct_detectors();
        // Detectors 0, 1 fired (score >= 0.5) and it was a true positive = correct
        assert!(correct[0]); // 0.8 >= 0.5, fired, TP = correct
        assert!(correct[1]); // 0.6 >= 0.5, fired, TP = correct
        assert!(!correct[2]); // 0.3 < 0.5, not fired, but was TP = incorrect
    }

    #[test]
    fn test_learning_update() {
        let events = vec![
            FeedbackEvent::true_positive(
                1,
                1000,
                [0.8, 0.6, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                FeedbackSource::LLMAnalysis,
                1.0,
            ),
            FeedbackEvent::false_positive(
                2,
                2000,
                [0.9, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                FeedbackSource::HumanReview,
                0.8,
            ),
        ];

        let update = LearningUpdate::from_batch(&events);
        assert!(update.is_significant());
        assert!(update.true_positives > 0);
        assert!(update.false_positives > 0);
    }
}
