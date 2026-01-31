pub mod adaptive_ensemble;
pub mod adaptive_threshold;
pub mod behavioral_fingerprint;
pub mod drift_detector;
pub mod enhanced_cusum;
pub mod ewma;
pub mod histogram;
pub mod hll;
pub mod holtwinters;
pub mod multi_scale;
pub mod rrcf;
pub mod spectral_residual;

// Re-exports for convenience
pub use adaptive_ensemble::{AdaptiveEnsemble, DetectorOutput};
pub use adaptive_threshold::{AdaptiveThreshold, ThresholdMethod};
pub use behavioral_fingerprint::{BehavioralFingerprintDetector, ProfileStore};
pub use drift_detector::{DriftType, EnsembleDriftDetector};
pub use enhanced_cusum::{CUSUM, EnhancedCUSUM};
pub use multi_scale::MultiScaleDetector;
pub use rrcf::{RRCFDetector, StreamingRRCF};
pub use spectral_residual::SpectralResidual;
