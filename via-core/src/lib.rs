use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_ulonglong};

pub mod algo;
pub mod engine;
pub mod engine_v2;
pub mod simulation;
pub mod tier2_ffi;

use engine::{AnomalyProfile, AnomalyResult};
use engine_v2::{AnomalyProfileV2, AnomalyResultV2};
use simulation::{
    scenarios::{
        performance::{CpuSpike, MemoryLeak},
        security::{CredentialStuffing, PortScan, SqlInjection},
        traffic::NormalTraffic,
    },
    SimulationEngine,
};

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
    value: c_double, // NEW: Latency or Payload Size
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

    let profile = unsafe { &mut *ptr };
    let result = profile.process(timestamp, str_slice, value);

    unsafe {
        *out_result = result;
    }
}

// --- Simulation FFI ---

#[unsafe(no_mangle)]
pub extern "C" fn create_simulation() -> *mut SimulationEngine {
    Box::into_raw(Box::new(SimulationEngine::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn free_simulation(ptr: *mut SimulationEngine) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_simulation(ptr: *mut SimulationEngine) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.clear_scenarios();
}

#[unsafe(no_mangle)]
pub extern "C" fn add_normal_traffic(ptr: *mut SimulationEngine, rps: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(NormalTraffic::new(rps)));
}

#[unsafe(no_mangle)]
pub extern "C" fn add_memory_leak(ptr: *mut SimulationEngine, leak_rate: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(MemoryLeak::new("payment-service", leak_rate)));
}

#[unsafe(no_mangle)]
pub extern "C" fn add_cpu_spike(ptr: *mut SimulationEngine, intensity: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(CpuSpike::new("recommendation-engine", intensity)));
}

#[unsafe(no_mangle)]
pub extern "C" fn add_credential_stuffing(ptr: *mut SimulationEngine, rps: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(CredentialStuffing { attack_rps: rps }));
}

#[unsafe(no_mangle)]
pub extern "C" fn add_sql_injection(ptr: *mut SimulationEngine, rps: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(SqlInjection { attack_rps: rps }));
}

#[unsafe(no_mangle)]
pub extern "C" fn add_port_scan(ptr: *mut SimulationEngine, rps: c_double) {
    if ptr.is_null() {
        return;
    }
    let sim = unsafe { &mut *ptr };
    sim.add_scenario(Box::new(PortScan {
        source_ip: "45.33.22.11".to_string(),
        scan_speed: rps,
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn simulation_tick(
    ptr: *mut SimulationEngine,
    delta_ns: c_ulonglong,
) -> *mut c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let sim = unsafe { &mut *ptr };
    let json = sim.tick_json(delta_ns);
    CString::new(json).unwrap().into_raw()
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

// --- Enhanced Anomaly Profile V2 FFI ---

#[unsafe(no_mangle)]
pub extern "C" fn create_profile_v2(
    hw_alpha: c_double,
    hw_beta: c_double,
    hw_gamma: c_double,
    period: usize,
    hist_bins: usize,
    min_val: c_double,
    max_val: c_double,
    hist_decay: c_double,
) -> *mut AnomalyProfileV2 {
    let profile = AnomalyProfileV2::new(
        hw_alpha, hw_beta, hw_gamma, period, hist_bins, min_val, max_val, hist_decay,
    );
    Box::into_raw(Box::new(profile))
}

#[unsafe(no_mangle)]
pub extern "C" fn create_profile_v2_default() -> *mut AnomalyProfileV2 {
    let profile = AnomalyProfileV2::default();
    Box::into_raw(Box::new(profile))
}

#[unsafe(no_mangle)]
pub extern "C" fn free_profile_v2(ptr: *mut AnomalyProfileV2) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(ptr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn process_event_v2(
    ptr: *mut AnomalyProfileV2,
    timestamp: c_ulonglong,
    unique_id: *const c_char,
    value: c_double,
    out_result: *mut AnomalyResultV2,
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
pub extern "C" fn reset_profile_v2(ptr: *mut AnomalyProfileV2) {
    if ptr.is_null() {
        return;
    }
    let profile = unsafe { &mut *ptr };
    profile.reset();
}
