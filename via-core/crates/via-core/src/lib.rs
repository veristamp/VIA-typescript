//! VIA-Core: SOTA Anomaly Detection Engine
//!
//! High-performance Tier-1 detection engine with:
//! - 10 SOTA detectors (Volume, Distribution, Cardinality, Burst, Spectral, ChangePoint, RRCF, MultiScale, Behavioral, Drift)
//! - Adaptive Ensemble with Thompson Sampling weight learning
//! - Rich AnomalySignal output with full attribution
//! - Feedback loop for continuous improvement
//! - Memory-bounded profile registry with LRU eviction
//! - Checkpoint/recovery for Bun-managed persistence

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_ulonglong};

// Core modules
pub mod algo;
pub mod checkpoint;
pub mod engine;
pub mod feedback;
pub mod registry;
pub mod signal;

// Re-exports
pub use checkpoint::{CheckpointError, CheckpointManager, CheckpointRequest, FullCheckpoint};
pub use engine::{AnomalyProfile, AnomalyResult, ProfileConfig, SignalContext};
pub use feedback::{FeedbackChannel, FeedbackEvent, FeedbackSource, FeedbackStats};
pub use registry::{ProfileRegistry, RegistryConfig};
pub use signal::{
    AnomalySignal, Attribution, BaselineSummary, DetectorId, DetectorScore, NUM_DETECTORS, Severity,
};

// ============================================================================
// FFI INTERFACE
// ============================================================================

/// Create a new anomaly profile with default configuration
#[unsafe(no_mangle)]
pub extern "C" fn via_create_profile() -> *mut AnomalyProfile {
    let profile = AnomalyProfile::default();
    Box::into_raw(Box::new(profile))
}

/// Create a new anomaly profile with custom parameters (legacy interface)
#[unsafe(no_mangle)]
pub extern "C" fn create_profile(
    hw_alpha: c_double,
    hw_beta: c_double,
    hw_gamma: c_double,
    period: usize,
    hist_bins: usize,
    min_val: c_double,
    max_val: c_double,
    hist_decay: c_double,
) -> *mut AnomalyProfile {
    let profile = AnomalyProfile::new(
        hw_alpha, hw_beta, hw_gamma, period, hist_bins, min_val, max_val, hist_decay,
    );
    Box::into_raw(Box::new(profile))
}

/// Free a profile
#[unsafe(no_mangle)]
pub extern "C" fn free_profile(ptr: *mut AnomalyProfile) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

/// Process an event and return legacy AnomalyResult (for backward compatibility)
#[unsafe(no_mangle)]
pub extern "C" fn process_event(
    ptr: *mut AnomalyProfile,
    timestamp: c_ulonglong,
    unique_id: *const c_char,
    value: c_double,
    out_result: *mut AnomalyResult,
) {
    if ptr.is_null() || unique_id.is_null() || out_result.is_null() {
        return;
    }

    let c_str = unsafe { CStr::from_ptr(unique_id) };
    let str_slice = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    let hash = xxhash_rust::xxh3::xxh3_64(str_slice.as_bytes());
    let profile = unsafe { &mut *ptr };
    let signal = profile.process_with_hash(timestamp, hash, value);
    let result: AnomalyResult = signal.into();

    unsafe {
        *out_result = result;
    }
}

/// Process an event and return full AnomalySignal (new interface)
///
/// Returns a heap-allocated AnomalySignal that must be freed with `via_free_signal`
#[unsafe(no_mangle)]
pub extern "C" fn via_process_event(
    ptr: *mut AnomalyProfile,
    timestamp: c_ulonglong,
    unique_id_hash: c_ulonglong,
    value: c_double,
) -> *mut AnomalySignal {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }

    let profile = unsafe { &mut *ptr };
    let signal = profile.process_with_hash(timestamp, unique_id_hash, value);

    Box::into_raw(Box::new(signal))
}

/// Free an AnomalySignal
#[unsafe(no_mangle)]
pub extern "C" fn via_free_signal(ptr: *mut AnomalySignal) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

/// Get signal fields (for FFI access without full struct copy)
#[unsafe(no_mangle)]
pub extern "C" fn via_signal_is_anomaly(ptr: *const AnomalySignal) -> bool {
    if ptr.is_null() {
        return false;
    }
    unsafe { (*ptr).is_anomaly }
}

#[unsafe(no_mangle)]
pub extern "C" fn via_signal_severity(ptr: *const AnomalySignal) -> u8 {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).severity as u8 }
}

#[unsafe(no_mangle)]
pub extern "C" fn via_signal_score(ptr: *const AnomalySignal) -> c_double {
    if ptr.is_null() {
        return 0.0;
    }
    unsafe { (*ptr).ensemble_score }
}

#[unsafe(no_mangle)]
pub extern "C" fn via_signal_confidence(ptr: *const AnomalySignal) -> c_double {
    if ptr.is_null() {
        return 0.0;
    }
    unsafe { (*ptr).confidence }
}

#[unsafe(no_mangle)]
pub extern "C" fn via_signal_primary_detector(ptr: *const AnomalySignal) -> u8 {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).attribution.primary_detector }
}

#[unsafe(no_mangle)]
pub extern "C" fn via_signal_detectors_fired(ptr: *const AnomalySignal) -> u8 {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).attribution.detectors_fired }
}

/// Get detector score by index
#[unsafe(no_mangle)]
pub extern "C" fn via_signal_detector_score(ptr: *const AnomalySignal, detector_idx: u8) -> f32 {
    if ptr.is_null() || detector_idx >= NUM_DETECTORS as u8 {
        return 0.0;
    }
    unsafe { (*ptr).detector_scores[detector_idx as usize].score }
}

/// Get detector weight by index
#[unsafe(no_mangle)]
pub extern "C" fn via_signal_detector_weight(ptr: *const AnomalySignal, detector_idx: u8) -> f32 {
    if ptr.is_null() || detector_idx >= NUM_DETECTORS as u8 {
        return 0.0;
    }
    unsafe { (*ptr).detector_weights[detector_idx as usize] }
}

/// Serialize signal to JSON (returns null-terminated string, must free with via_free_string)
#[unsafe(no_mangle)]
pub extern "C" fn via_signal_to_json(ptr: *const AnomalySignal) -> *mut c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }

    let signal = unsafe { &*ptr };
    match serde_json::to_string(signal) {
        Ok(json) => match CString::new(json) {
            Ok(c_str) => c_str.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Reset a profile
#[unsafe(no_mangle)]
pub extern "C" fn reset_profile(ptr: *mut AnomalyProfile) {
    if ptr.is_null() {
        return;
    }
    let profile = unsafe { &mut *ptr };
    profile.reset();
}

/// Free a string allocated by Rust
#[unsafe(no_mangle)]
pub extern "C" fn free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

/// Alias for backward compatibility
#[unsafe(no_mangle)]
pub extern "C" fn via_free_string(s: *mut c_char) {
    free_string(s);
}

// ============================================================================
// FEEDBACK FFI
// ============================================================================

/// Send feedback to a profile (for weight learning)
#[unsafe(no_mangle)]
pub extern "C" fn via_send_feedback(
    profile_ptr: *mut AnomalyProfile,
    entity_hash: c_ulonglong,
    signal_timestamp: c_ulonglong,
    was_true_positive: bool,
    detector_scores: *const f32,
    feedback_source: u8,
    confidence: f32,
) -> bool {
    if profile_ptr.is_null() || detector_scores.is_null() {
        return false;
    }

    let profile = unsafe { &mut *profile_ptr };

    // Copy detector scores
    let scores: [f32; NUM_DETECTORS] = unsafe {
        let mut arr = [0.0f32; NUM_DETECTORS];
        for i in 0..NUM_DETECTORS {
            arr[i] = *detector_scores.add(i);
        }
        arr
    };

    let source = match feedback_source {
        0 => FeedbackSource::LLMAnalysis,
        1 => FeedbackSource::HumanReview,
        2 => FeedbackSource::AutoCorrelation,
        _ => FeedbackSource::Timeout,
    };

    let event = if was_true_positive {
        FeedbackEvent::true_positive(entity_hash, signal_timestamp, scores, source, confidence)
    } else {
        FeedbackEvent::false_positive(entity_hash, signal_timestamp, scores, source, confidence)
    };

    profile.apply_feedback(&[event]);
    true
}

// ============================================================================
// CHECKPOINT FFI
// ============================================================================

/// Create a checkpoint from a profile (returns JSON string, must free with via_free_string)
#[unsafe(no_mangle)]
pub extern "C" fn via_create_checkpoint(profile_ptr: *const AnomalyProfile) -> *mut c_char {
    if profile_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let profile = unsafe { &*profile_ptr };
    let checkpoint_data = profile.to_checkpoint();

    // Return as base64-encoded string for easy transport
    let base64 = base64_encode(&checkpoint_data);
    match CString::new(base64) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Restore a profile from checkpoint (base64-encoded string)
#[unsafe(no_mangle)]
pub extern "C" fn via_restore_from_checkpoint(
    checkpoint_b64: *const c_char,
) -> *mut AnomalyProfile {
    if checkpoint_b64.is_null() {
        return std::ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(checkpoint_b64) };
    let b64_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let data = match base64_decode(b64_str) {
        Some(d) => d,
        None => return std::ptr::null_mut(),
    };

    match AnomalyProfile::from_checkpoint(&data) {
        Ok(profile) => Box::into_raw(Box::new(profile)),
        Err(_) => std::ptr::null_mut(),
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Hash a string (for pre-hashing on the Bun side)
#[unsafe(no_mangle)]
pub extern "C" fn via_hash_string(s: *const c_char) -> c_ulonglong {
    if s.is_null() {
        return 0;
    }

    let c_str = unsafe { CStr::from_ptr(s) };
    match c_str.to_str() {
        Ok(str_slice) => xxhash_rust::xxh3::xxh3_64(str_slice.as_bytes()),
        Err(_) => 0,
    }
}

/// Get detector name by index
#[unsafe(no_mangle)]
pub extern "C" fn via_detector_name(idx: u8) -> *const c_char {
    static NAMES: [&str; NUM_DETECTORS] = [
        "Volume/RPS\0",
        "Distribution/Value\0",
        "Cardinality/Velocity\0",
        "Burst/IAT\0",
        "Spectral/FFT\0",
        "ChangePoint/Trend\0",
        "RRCF/Isolation\0",
        "MultiScale/Temporal\0",
        "Behavioral/Fingerprint\0",
        "Drift/Concept\0",
    ];

    if idx >= NUM_DETECTORS as u8 {
        return std::ptr::null();
    }

    NAMES[idx as usize].as_ptr() as *const c_char
}

/// Get the number of detectors
#[unsafe(no_mangle)]
pub extern "C" fn via_num_detectors() -> u8 {
    NUM_DETECTORS as u8
}

// ============================================================================
// BASE64 HELPERS (simple implementation for checkpoint transport)
// ============================================================================

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(BASE64_CHARS[b0 >> 2] as char);
        result.push(BASE64_CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((b1 & 0x0F) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[b2 & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut result = Vec::with_capacity(s.len() * 3 / 4);

    let decode_char = |c: char| -> Option<u8> {
        match c {
            'A'..='Z' => Some(c as u8 - b'A'),
            'a'..='z' => Some(c as u8 - b'a' + 26),
            '0'..='9' => Some(c as u8 - b'0' + 52),
            '+' => Some(62),
            '/' => Some(63),
            _ => None,
        }
    };

    let chars: Vec<u8> = s.chars().filter_map(decode_char).collect();

    for chunk in chars.chunks(4) {
        if chunk.len() >= 2 {
            result.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            result.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() >= 4 {
            result.push((chunk[2] << 6) | chunk[3]);
        }
    }

    Some(result)
}

use crate::checkpoint::Checkpointable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let original = b"Hello, World!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_ffi_profile_lifecycle() {
        let profile = via_create_profile();
        assert!(!profile.is_null());

        let signal = via_process_event(profile, 1000000, 12345, 100.0);
        assert!(!signal.is_null());

        let is_anomaly = via_signal_is_anomaly(signal);
        assert!(!is_anomaly); // Warmup period

        via_free_signal(signal);
        free_profile(profile);
    }

    #[test]
    fn test_detector_names() {
        assert!(!via_detector_name(0).is_null());
        assert!(via_detector_name(100).is_null());
        assert_eq!(via_num_detectors(), 10);
    }
}
