use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_ulonglong};

pub mod algo;
pub mod engine;

use engine::{AnomalyProfile, AnomalyResult};

// --- Anomaly Profile FFI ---

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

#[unsafe(no_mangle)]
pub extern "C" fn free_profile(ptr: *mut AnomalyProfile) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

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
    let result = profile.process_with_hash(timestamp, hash, value);

    unsafe {
        *out_result = result;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_profile(ptr: *mut AnomalyProfile) {
    if ptr.is_null() {
        return;
    }
    let profile = unsafe { &mut *ptr };
    profile.reset();
}

#[unsafe(no_mangle)]
pub extern "C" fn free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}
