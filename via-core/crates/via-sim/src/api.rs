//! HTTP Control API for Simulation
//!
//! Provides a REST API for controlling the simulation in real-time:
//! - Start/stop/pause simulation
//! - Inject anomalies
//! - Get status and metrics
//! - Dashboard data streaming

use crate::core::{GroundTruth, SimulationBatch};
use crate::engine::{EngineState, SimulationEngine};
use crate::scenarios;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// HTTP API Server Configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Host to bind to (default: 127.0.0.1)
    pub host: String,
    /// Port to listen on (default: 8080)
    pub port: u16,
    /// Enable CORS (default: true)
    pub cors_enabled: bool,
    /// Tick interval in milliseconds (default: 100)
    pub tick_interval_ms: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_enabled: true,
            tick_interval_ms: 100,
        }
    }
}

/// Shared state for the simulation API
pub struct SimulationState {
    pub engine: SimulationEngine,
    pub config: ApiConfig,
    pub tick_count: u64,
}

impl SimulationState {
    pub fn new(config: ApiConfig) -> Self {
        Self {
            engine: SimulationEngine::new(),
            config,
            tick_count: 0,
        }
    }
}

/// Thread-safe handle to simulation state
pub type SharedState = Arc<Mutex<SimulationState>>;

/// Create a new shared state instance
pub fn create_shared_state(config: ApiConfig) -> SharedState {
    Arc::new(Mutex::new(SimulationState::new(config)))
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Request to start simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRequest {
    pub scenario: String,
    #[serde(default = "default_intensity")]
    pub intensity: f64,
}

fn default_intensity() -> f64 {
    1.0
}

/// Request to inject an anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectAnomalyRequest {
    pub anomaly_type: String,
    #[serde(default = "default_duration_ms")]
    pub duration_ms: u64,
}

fn default_duration_ms() -> u64 {
    30_000 // 30 seconds
}

/// Generic API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

/// Available scenarios list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenariosResponse {
    pub scenarios: Vec<ScenarioInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioInfo {
    pub name: String,
    pub description: String,
}

/// Simulation status (replaces old SimulationStatus from live_types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct SimulationStatus {
    pub isRunning: bool,
    pub isPaused: bool,
    pub currentScenario: String,
    pub eventsGenerated: u64,
    pub anomalyLogsGenerated: u64,
    pub tickCount: u64,
    pub elapsedMs: u64,
    pub activeGroundTruth: Vec<GroundTruth>,
}

impl SimulationStatus {
    pub fn from_engine(engine: &SimulationEngine) -> Self {
        let stats = engine.stats();
        Self {
            isRunning: engine.state() == EngineState::Running,
            isPaused: engine.state() == EngineState::Paused,
            currentScenario: "active".to_string(), // TODO: track in engine
            eventsGenerated: stats.total_logs,
            anomalyLogsGenerated: stats.total_anomaly_logs,
            tickCount: stats.tick_count,
            elapsedMs: engine.elapsed() / 1_000_000,
            activeGroundTruth: Vec::new(), // Would need to expose from engine
        }
    }
}

/// Dashboard state for UI
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardState {
    pub timestamp: u64,
    pub is_simulating: bool,
    pub events_per_second: f64,
    pub total_events: u64,
    pub anomaly_events: u64,
    pub tick_count: u64,
    pub elapsed_ms: u64,
    pub active_scenarios: Vec<String>,
    pub ground_truth: Vec<GroundTruth>,
}

impl DashboardState {
    pub fn from_batch(batch: &SimulationBatch, engine: &SimulationEngine) -> Self {
        Self {
            timestamp: batch.metadata.timestamp_ns,
            is_simulating: engine.state() == EngineState::Running,
            events_per_second: batch.metadata.log_count as f64
                / (batch.metadata.elapsed_ns as f64 / 1_000_000_000.0).max(0.001),
            total_events: batch.metadata.log_count,
            anomaly_events: batch.metadata.anomaly_log_count,
            tick_count: engine.stats().tick_count,
            elapsed_ms: batch.metadata.elapsed_ns / 1_000_000,
            active_scenarios: batch.metadata.active_scenarios.clone(),
            ground_truth: batch.ground_truth.clone(),
        }
    }
}

// ============================================================================
// API Handler Functions (for integration with any HTTP framework)
// ============================================================================

/// Handle GET /scenarios - list available scenarios
pub fn handle_list_scenarios() -> ApiResponse<ScenariosResponse> {
    let scenarios: Vec<ScenarioInfo> = scenarios::list_scenarios()
        .into_iter()
        .map(|(name, desc)| ScenarioInfo {
            name: name.to_string(),
            description: desc.to_string(),
        })
        .collect();

    ApiResponse::success(ScenariosResponse { scenarios })
}

/// Handle POST /start - start simulation
pub fn handle_start(state: &SharedState, request: StartRequest) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    state.engine.start(&request.scenario);

    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle POST /stop - stop simulation
pub fn handle_stop(state: &SharedState) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    state.engine.stop();

    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle POST /pause - pause simulation
pub fn handle_pause(state: &SharedState) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    state.engine.pause();

    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle POST /resume - resume simulation
pub fn handle_resume(state: &SharedState) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    state.engine.resume();

    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle POST /inject - inject an anomaly
pub fn handle_inject_anomaly(
    state: &SharedState,
    request: InjectAnomalyRequest,
) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    let result = state
        .engine
        .inject_anomaly(&request.anomaly_type, request.duration_ms);

    if result.is_some() {
        let status = SimulationStatus::from_engine(&state.engine);
        ApiResponse::success(status)
    } else {
        ApiResponse::error(&format!("Unknown anomaly type: {}", request.anomaly_type))
    }
}

/// Handle GET /status - get current simulation status
pub fn handle_get_status(state: &SharedState) -> ApiResponse<SimulationStatus> {
    let state = state.lock().unwrap();
    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle GET /dashboard - get full dashboard state
pub fn handle_get_dashboard(state: &SharedState) -> ApiResponse<DashboardState> {
    let mut state = state.lock().unwrap();
    // Generate a quick tick to get current data
    let batch = state.engine.tick(0);
    let dashboard = DashboardState::from_batch(&batch, &state.engine);
    ApiResponse::success(dashboard)
}

/// Handle POST /tick - advance simulation by one tick (for manual control)
pub fn handle_tick(state: &SharedState, delta_ms: u64) -> ApiResponse<SimulationBatch> {
    let mut state = state.lock().unwrap();
    let batch = state.engine.tick_ms(delta_ms);
    state.tick_count += 1;

    ApiResponse::success(batch)
}

/// Handle POST /rate - change simulation speed (placeholder - rate not implemented yet)
pub fn handle_change_rate(
    state: &SharedState,
    _events_per_second: f64,
) -> ApiResponse<SimulationStatus> {
    let state = state.lock().unwrap();
    // TODO: Implement rate control in engine
    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

/// Handle POST /reset - reset all state
pub fn handle_reset(state: &SharedState) -> ApiResponse<SimulationStatus> {
    let mut state = state.lock().unwrap();

    state.engine.reset();

    let status = SimulationStatus::from_engine(&state.engine);
    ApiResponse::success(status)
}

// ============================================================================
// API Documentation
// ============================================================================

/// API routes definition for documentation/integration
pub fn get_api_routes() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("GET", "/scenarios", "List all available scenarios"),
        ("GET", "/status", "Get current simulation status"),
        ("GET", "/dashboard", "Get full dashboard state with metrics"),
        ("POST", "/start", "Start simulation with scenario"),
        ("POST", "/stop", "Stop the simulation"),
        ("POST", "/pause", "Pause the simulation"),
        ("POST", "/resume", "Resume paused simulation"),
        ("POST", "/inject", "Inject an anomaly"),
        ("POST", "/tick", "Manually advance simulation (debug)"),
        ("POST", "/reset", "Reset all state"),
    ]
}

/// Print API documentation to stdout
pub fn print_api_docs(config: &ApiConfig) {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║           VIA Simulation HTTP Control API                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Base URL: http://{}:{:<39} ║", config.host, config.port);
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ ENDPOINTS                                                    ║");
    println!("╠──────────────────────────────────────────────────────────────╣");

    for (method, path, desc) in get_api_routes() {
        println!("║ {:6} {:12} - {:40} ║", method, path, desc);
    }

    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ EXAMPLE USAGE                                                ║");
    println!("╠──────────────────────────────────────────────────────────────╣");
    println!("║ # Start simulation                                           ║");
    println!(
        "║ curl -X POST http://{}:{}/start \\                ║",
        config.host, config.port
    );
    println!("║   -H 'Content-Type: application/json' \\                      ║");
    println!("║   -d '{{\"scenario\": \"normal_traffic\"}}'                       ║");
    println!("║                                                              ║");
    println!("║ # Inject anomaly                                             ║");
    println!(
        "║ curl -X POST http://{}:{}/inject \\               ║",
        config.host, config.port
    );
    println!("║   -H 'Content-Type: application/json' \\                      ║");
    println!("║   -d '{{\"anomaly_type\": \"memory_leak\", \"duration_ms\": 30000}}'║");
    println!("║                                                              ║");
    println!("║ # Get tick data                                              ║");
    println!(
        "║ curl -X POST http://{}:{}/tick?delta_ms=100      ║",
        config.host, config.port
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_scenarios() {
        let response = handle_list_scenarios();
        assert!(response.success);
        assert!(response.data.is_some());
        let data = response.data.unwrap();
        assert!(!data.scenarios.is_empty());
    }

    #[test]
    fn test_start_stop_cycle() {
        let state = create_shared_state(ApiConfig::default());

        // Start
        let start_response = handle_start(
            &state,
            StartRequest {
                scenario: "normal_traffic".to_string(),
                intensity: 1.0,
            },
        );
        assert!(start_response.success);

        // Check running
        let status = handle_get_status(&state);
        assert!(status.success);
        assert!(status.data.unwrap().isRunning);

        // Stop
        let stop_response = handle_stop(&state);
        assert!(stop_response.success);
    }

    #[test]
    fn test_inject_anomaly() {
        let state = create_shared_state(ApiConfig::default());

        // Start simulation first
        handle_start(
            &state,
            StartRequest {
                scenario: "normal_traffic".to_string(),
                intensity: 1.0,
            },
        );

        // Inject anomaly
        let inject_response = handle_inject_anomaly(
            &state,
            InjectAnomalyRequest {
                anomaly_type: "memory_leak".to_string(),
                duration_ms: 30000,
            },
        );
        assert!(inject_response.success);
    }
}
