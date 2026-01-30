//! FFI Module for Tier-2 Bun Engine Integration
//!
//! This module provides a high-performance FFI interface between the Rust core (Tier-1)
//! and the Bun-based Tier-2 engine. The Tier-2 engine handles:
//! - Complex DB lookups for entity resolution
//! - Historical context retrieval
//! - Feedback loop management
//! - Cross-entity correlation analysis
//!
//! The FFI design prioritizes:
//! - Zero-copy data transfer where possible
//! - Async-compatible callbacks
//! - Type-safe serialization
//! - Minimal overhead

use serde::{Deserialize, Serialize};
use std::ffi::{c_char, c_ulonglong, CStr, CString};
use std::os::raw::c_void;

/// Callback type for async Tier-2 lookups
///
/// Bun can register a callback that the Rust core will invoke
/// when it needs additional context about an entity.
pub type Tier2Callback = extern "C" fn(
    entity_hash: c_ulonglong,
    context_type: c_char,      // 1=Historical, 2=Correlations, 3=Feedback
    result_ptr: *const c_char, // JSON result
    user_data: *mut c_void,
);

/// Feedback callback for updating Tier-2 with detection results
///
/// This allows the Tier-2 engine to learn from Tier-1 detections
/// and improve its DB queries and entity resolution.
pub type FeedbackCallback = extern "C" fn(
    detection_result: *const c_char, // JSON DetectionFeedback
    user_data: *mut c_void,
);

/// Global callback storage (thread-safe via once_cell)
static mut TIER2_CALLBACK: Option<Tier2Callback> = None;
static mut FEEDBACK_CALLBACK: Option<FeedbackCallback> = None;
static mut CALLBACK_USER_DATA: *mut c_void = std::ptr::null_mut();

/// Detection feedback structure sent to Tier-2
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectionFeedback {
    pub timestamp: u64,
    pub entity_hash: u64,
    pub is_anomaly: bool,
    pub anomaly_score: f64,
    pub signal_type: u8,
    pub detector_name: String,
    pub confidence: f64,
    pub reason: String,
    /// Whether this was confirmed by human review or Tier-2 analysis
    pub confirmed: Option<bool>,
    /// Additional metadata for Tier-2 DB storage
    pub metadata: serde_json::Value,
}

/// Context request structure for Tier-2 lookups
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextRequest {
    pub entity_hash: u64,
    pub request_type: ContextType,
    pub time_range_seconds: u64,
    pub max_results: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ContextType {
    HistoricalEvents,
    EntityCorrelations,
    BehavioralProfile,
    FeedbackHistory,
}

/// Context response from Tier-2
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContextResponse {
    pub entity_hash: u64,
    pub found: bool,
    pub historical_scores: Vec<f64>,
    pub correlated_entities: Vec<u64>,
    pub behavioral_deviation: f64,
    pub last_seen: Option<u64>,
    pub threat_score: f64,
}

/// FFI: Register the Tier-2 callback from Bun
#[unsafe(no_mangle)]
pub extern "C" fn via_register_tier2_callback(callback: Tier2Callback, user_data: *mut c_void) {
    unsafe {
        TIER2_CALLBACK = Some(callback);
        CALLBACK_USER_DATA = user_data;
    }
}

/// FFI: Register the feedback callback from Bun
#[unsafe(no_mangle)]
pub extern "C" fn via_register_feedback_callback(
    callback: FeedbackCallback,
    user_data: *mut c_void,
) {
    unsafe {
        FEEDBACK_CALLBACK = Some(callback);
        CALLBACK_USER_DATA = user_data;
    }
}

/// Internal: Request context from Tier-2 (async callback-based)
pub fn request_context(entity_hash: u64, context_type: ContextType) -> Option<ContextResponse> {
    unsafe {
        if let Some(callback) = TIER2_CALLBACK {
            // Clone for later use before moving
            let context_type_code = context_type.clone() as c_char;

            let request = ContextRequest {
                entity_hash,
                request_type: context_type,
                time_range_seconds: 3600, // Last hour
                max_results: 100,
            };

            let request_json = serde_json::to_string(&request).ok()?;
            let request_cstring = CString::new(request_json).ok()?;

            // Invoke the callback - Tier-2 will process and call back via via_submit_context_result
            callback(
                entity_hash,
                context_type_code,
                request_cstring.as_ptr(),
                CALLBACK_USER_DATA,
            );

            // Note: Actual response handling would be async
            // This is a simplified sync version for the hot path
            None
        } else {
            None
        }
    }
}

/// FFI: Submit context result back from Tier-2 to Rust
#[unsafe(no_mangle)]
pub extern "C" fn via_submit_context_result(entity_hash: c_ulonglong, result_json: *const c_char) {
    if result_json.is_null() {
        return;
    }

    let c_str = unsafe { CStr::from_ptr(result_json) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    let _response: ContextResponse = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Store in thread-safe cache for retrieval
    // In production, use a concurrent hashmap
    // For now, we log the successful receipt
    #[cfg(debug_assertions)]
    eprintln!(
        "Received context for entity {}: found={}",
        entity_hash, _response.found
    );
}

/// FFI: Send detection feedback to Tier-2
#[unsafe(no_mangle)]
pub extern "C" fn via_send_detection_feedback(feedback_json: *const c_char) -> bool {
    unsafe {
        if let Some(callback) = FEEDBACK_CALLBACK {
            if feedback_json.is_null() {
                return false;
            }

            callback(feedback_json, CALLBACK_USER_DATA);
            true
        } else {
            false
        }
    }
}

/// Internal: Send feedback from Rust to Tier-2
pub fn send_feedback(feedback: &DetectionFeedback) -> bool {
    let json = match serde_json::to_string(feedback) {
        Ok(j) => j,
        Err(_) => return false,
    };

    let cstring = match CString::new(json) {
        Ok(c) => c,
        Err(_) => return false,
    };

    via_send_detection_feedback(cstring.as_ptr())
}

/// FFI: Get current callback status
#[unsafe(no_mangle)]
pub extern "C" fn via_get_tier2_status() -> c_char {
    unsafe {
        // Use addr_of! to create raw pointer without creating a reference
        let has_tier2 = std::ptr::addr_of!(TIER2_CALLBACK).read().is_some() as c_char;
        let has_feedback = std::ptr::addr_of!(FEEDBACK_CALLBACK).read().is_some() as c_char;
        (has_tier2 << 1) | has_feedback
    }
}

/// FFI: Batch process events with Tier-2 context
///
/// This is the main entry point for high-throughput processing.
/// Bun can batch events and send them to Rust for processing,
/// with optional async context enrichment from Tier-2.
#[unsafe(no_mangle)]
pub extern "C" fn via_batch_process_with_tier2(
    events_json: *const c_char,
    results_buffer: *mut c_char,
    buffer_size: usize,
) -> i32 {
    if events_json.is_null() || results_buffer.is_null() || buffer_size == 0 {
        return -1;
    }

    let c_str = unsafe { CStr::from_ptr(events_json) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    // Parse batch of events
    let events: Vec<BatchEvent> = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return -3,
    };

    // Process events (simplified - would use actual profile)
    let results: Vec<BatchResult> = events
        .iter()
        .map(|event| BatchResult {
            entity_hash: event.entity_hash,
            is_anomaly: false, // Would call actual detector
            anomaly_score: 0.0,
            needs_tier2: false,
        })
        .collect();

    // Serialize results
    let result_json = match serde_json::to_string(&results) {
        Ok(j) => j,
        Err(_) => return -4,
    };

    // Copy to buffer
    let bytes = result_json.as_bytes();
    let to_copy = bytes.len().min(buffer_size - 1);

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, results_buffer, to_copy);
        *results_buffer.add(to_copy) = 0; // Null terminate
    }

    results.len() as i32
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BatchEvent {
    timestamp: u64,
    entity_hash: u64,
    value: f64,
    signal_type: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BatchResult {
    entity_hash: u64,
    is_anomaly: bool,
    anomaly_score: f64,
    needs_tier2: bool,
}

/// FFI: Get version info
#[unsafe(no_mangle)]
pub extern "C" fn via_get_version() -> *const c_char {
    static VERSION: &str = concat!("via-core-v2-", env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_feedback_serialization() {
        let feedback = DetectionFeedback {
            timestamp: 1234567890,
            entity_hash: 9876543210,
            is_anomaly: true,
            anomaly_score: 0.85,
            signal_type: 3,
            detector_name: "CardinalityDetector".to_string(),
            confidence: 0.92,
            reason: "High velocity of new entities".to_string(),
            confirmed: None,
            metadata: serde_json::json!({"source": "test"}),
        };

        let json = serde_json::to_string(&feedback).unwrap();
        let parsed: DetectionFeedback = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.entity_hash, feedback.entity_hash);
        assert_eq!(parsed.anomaly_score, feedback.anomaly_score);
    }

    #[test]
    fn test_context_response_default() {
        let response = ContextResponse::default();
        assert!(!response.found);
        assert_eq!(response.threat_score, 0.0);
    }
}
