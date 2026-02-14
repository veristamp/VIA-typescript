//! Checkpoint and Recovery for Bun-Managed Persistence
//!
//! This module handles serialization of profile states for persistence.
//! Tier-2 (Bun) owns the storage; Tier-1 just serializes/deserializes.

use crate::policy::runtime as policy_runtime;
use crate::registry::ProfileRegistry;
use crate::signal::NUM_DETECTORS;
use serde::{Deserialize, Serialize};

/// Version for checkpoint format migrations
pub const CHECKPOINT_VERSION: u32 = 1;

/// Serialized state for adaptive ensemble weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleCheckpoint {
    /// Current detector weights
    pub weights: [f64; NUM_DETECTORS],
    /// Thompson sampling alpha values
    pub alpha: [f64; NUM_DETECTORS],
    /// Thompson sampling beta values
    pub beta: [f64; NUM_DETECTORS],
    /// Total samples processed
    pub total_samples: u64,
}

impl Default for EnsembleCheckpoint {
    fn default() -> Self {
        Self {
            weights: [0.1; NUM_DETECTORS],
            alpha: [1.0; NUM_DETECTORS],
            beta: [1.0; NUM_DETECTORS],
            total_samples: 0,
        }
    }
}

/// Serialized state for a single detector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorCheckpoint {
    /// Detector type identifier
    pub detector_id: u8,
    /// Detector-specific state as opaque bytes
    pub state: Vec<u8>,
}

/// Serialized state for an anomaly profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCheckpoint {
    /// Entity hash
    pub entity_hash: u64,
    /// Total events processed
    pub event_count: u64,
    /// Priority level
    pub priority: u8,
    /// Ensemble state
    pub ensemble: EnsembleCheckpoint,
    /// Per-detector states
    pub detectors: Vec<DetectorCheckpoint>,
    /// Creation timestamp (nanoseconds)
    pub created_at: u64,
    /// Last access timestamp (nanoseconds)
    pub last_access: u64,
}

/// Full checkpoint containing all profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCheckpoint {
    /// Format version
    pub version: u32,
    /// Checkpoint timestamp
    pub timestamp: u64,
    /// Total profiles in checkpoint
    pub profile_count: usize,
    /// Individual profile checkpoints
    pub profiles: Vec<ProfileCheckpoint>,
    /// Global ensemble state (for new profiles)
    pub global_ensemble: EnsembleCheckpoint,
    /// Feedback statistics
    pub feedback_stats: FeedbackCheckpoint,
    /// Active runtime policy metadata
    #[serde(default)]
    pub policy: PolicyCheckpoint,
}

/// Checkpoint of feedback statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedbackCheckpoint {
    pub total_received: u64,
    pub total_processed: u64,
    pub true_positives: u64,
    pub false_positives: u64,
    pub false_negatives: u64,
}

/// Checkpointed policy metadata for deterministic policy-aware restart flow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyCheckpoint {
    pub active_policy_version: String,
    pub policy_checksum: u64,
}

impl FullCheckpoint {
    /// Create an empty checkpoint
    pub fn empty() -> Self {
        Self {
            version: CHECKPOINT_VERSION,
            timestamp: 0,
            profile_count: 0,
            profiles: Vec::new(),
            global_ensemble: EnsembleCheckpoint::default(),
            feedback_stats: FeedbackCheckpoint::default(),
            policy: PolicyCheckpoint::default(),
        }
    }

    /// Serialize to bytes (for sending to Bun)
    pub fn to_bytes(&self) -> Result<Vec<u8>, CheckpointError> {
        bincode::serialize(self).map_err(|e| CheckpointError::SerializationFailed(e.to_string()))
    }

    /// Deserialize from bytes (received from Bun)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CheckpointError> {
        let checkpoint: FullCheckpoint = bincode::deserialize(bytes)
            .map_err(|e| CheckpointError::DeserializationFailed(e.to_string()))?;

        // Version check
        if checkpoint.version > CHECKPOINT_VERSION {
            return Err(CheckpointError::UnsupportedVersion {
                found: checkpoint.version,
                max_supported: CHECKPOINT_VERSION,
            });
        }

        Ok(checkpoint)
    }

    /// Get size in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        self.to_bytes().map(|b| b.len()).unwrap_or(0)
    }
}

/// Errors that can occur during checkpoint operations
#[derive(Debug, Clone)]
pub enum CheckpointError {
    SerializationFailed(String),
    DeserializationFailed(String),
    UnsupportedVersion { found: u32, max_supported: u32 },
    ProfileNotFound(u64),
    InvalidState(String),
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializationFailed(e) => write!(f, "Serialization failed: {}", e),
            Self::DeserializationFailed(e) => write!(f, "Deserialization failed: {}", e),
            Self::UnsupportedVersion {
                found,
                max_supported,
            } => {
                write!(
                    f,
                    "Unsupported checkpoint version: {} (max supported: {})",
                    found, max_supported
                )
            }
            Self::ProfileNotFound(h) => write!(f, "Profile not found: {}", h),
            Self::InvalidState(e) => write!(f, "Invalid state: {}", e),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Request to create a checkpoint (sent to Tier-2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRequest {
    /// Unique checkpoint ID
    pub checkpoint_id: u64,
    /// Timestamp of request
    pub timestamp: u64,
    /// Serialized checkpoint data
    pub data: Vec<u8>,
    /// Number of profiles
    pub profile_count: usize,
    /// Uncompressed size
    pub uncompressed_size: usize,
}

/// Response from Tier-2 after storing checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResponse {
    /// Checkpoint ID that was stored
    pub checkpoint_id: u64,
    /// Whether storage succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Storage location (opaque to Tier-1)
    pub storage_key: Option<String>,
}

/// Request to recover from checkpoint (sent from Tier-2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRequest {
    /// Checkpoint ID to recover
    pub checkpoint_id: u64,
    /// Serialized checkpoint data
    pub data: Vec<u8>,
}

/// Trait for types that can be checkpointed
pub trait Checkpointable {
    /// Get checkpoint data
    fn to_checkpoint(&self) -> Vec<u8>;
    /// Restore from checkpoint data
    fn from_checkpoint(data: &[u8]) -> Result<Self, CheckpointError>
    where
        Self: Sized;
}

/// Manager for checkpoint operations
pub struct CheckpointManager {
    /// Auto-increment ID
    next_id: u64,
    /// Last successful checkpoint
    last_checkpoint_id: Option<u64>,
    /// Last checkpoint timestamp
    last_checkpoint_time: Option<u64>,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            last_checkpoint_id: None,
            last_checkpoint_time: None,
        }
    }

    /// Create a checkpoint (returns bytes to send to Tier-2)
    pub fn create_checkpoint<P: Checkpointable>(
        &mut self,
        registry: &ProfileRegistry<P>,
        global_ensemble: EnsembleCheckpoint,
        feedback_stats: FeedbackCheckpoint,
    ) -> Result<CheckpointRequest, CheckpointError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let mut profiles = Vec::with_capacity(registry.len());

        for (&hash, entry) in registry.iter_entries() {
            let state = entry.profile.to_checkpoint();
            profiles.push(ProfileCheckpoint {
                entity_hash: hash,
                event_count: entry.meta.event_count,
                priority: entry.meta.priority,
                ensemble: EnsembleCheckpoint::default(), // Per-profile ensemble if needed
                detectors: vec![DetectorCheckpoint {
                    detector_id: 0,
                    state,
                }],
                created_at: 0, // TODO: track creation time
                last_access: 0,
            });
        }

        let full = FullCheckpoint {
            version: CHECKPOINT_VERSION,
            timestamp,
            profile_count: profiles.len(),
            profiles,
            global_ensemble,
            feedback_stats,
            policy: PolicyCheckpoint {
                active_policy_version: policy_runtime().current_version(),
                policy_checksum: xxhash_rust::xxh3::xxh3_64(
                    policy_runtime().current_version().as_bytes(),
                ),
            },
        };

        let data = full.to_bytes()?;
        let uncompressed_size = data.len();

        let checkpoint_id = self.next_id;
        self.next_id += 1;

        Ok(CheckpointRequest {
            checkpoint_id,
            timestamp,
            data,
            profile_count: full.profile_count,
            uncompressed_size,
        })
    }

    /// Record successful checkpoint
    pub fn record_success(&mut self, checkpoint_id: u64) {
        self.last_checkpoint_id = Some(checkpoint_id);
        self.last_checkpoint_time = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        );
    }

    /// Get last successful checkpoint info
    pub fn last_checkpoint(&self) -> Option<(u64, u64)> {
        match (self.last_checkpoint_id, self.last_checkpoint_time) {
            (Some(id), Some(time)) => Some((id, time)),
            _ => None,
        }
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_serialization() {
        let checkpoint = FullCheckpoint {
            version: CHECKPOINT_VERSION,
            timestamp: 1234567890,
            profile_count: 0,
            profiles: vec![],
            global_ensemble: EnsembleCheckpoint::default(),
            feedback_stats: FeedbackCheckpoint::default(),
            policy: PolicyCheckpoint::default(),
        };

        let bytes = checkpoint.to_bytes().unwrap();
        let restored = FullCheckpoint::from_bytes(&bytes).unwrap();

        assert_eq!(restored.version, CHECKPOINT_VERSION);
        assert_eq!(restored.timestamp, 1234567890);
    }

    #[test]
    fn test_version_check() {
        let mut checkpoint = FullCheckpoint::empty();
        checkpoint.version = 999;

        let bytes = bincode::serialize(&checkpoint).unwrap();
        let result = FullCheckpoint::from_bytes(&bytes);

        assert!(matches!(
            result,
            Err(CheckpointError::UnsupportedVersion { .. })
        ));
    }
}
