//! # via-sim - SOTA OTel Log Simulation Engine
//!
//! Real-time OpenTelemetry log generation with controlled anomaly injection
//! and ground truth tracking for benchmarking anomaly detection systems.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         via-sim                                      │
//! │                                                                      │
//! │   ┌─────────────────────────────────────────────────────────────┐   │
//! │   │                  SimulationEngine                            │   │
//! │   │                                                             │   │
//! │   │  ┌─────────────┐  ┌─────────────┐  ┌────────────────────┐  │   │
//! │   │  │  Scenarios  │  │  Scheduler  │  │   Ground Truth      │  │   │
//! │   │  │  (plugins)  │──│  (timing)   │──│   (tracking)        │  │   │
//! │   │  └─────────────┘  └─────────────┘  └────────────────────┘  │   │
//! │   │         │                │                    │            │   │
//! │   │         └────────────────┼────────────────────┘            │   │
//! │   │                          ▼                                  │   │
//! │   │               ┌──────────────────────┐                      │   │
//! │   │               │   SimulationBatch    │                      │   │
//! │   │               │  (logs + ground_truth)                      │   │
//! │   │               └──────────────────────┘                      │   │
//! │   └─────────────────────────────────────────────────────────────┘   │
//! │                                                                      │
//! │   Scenarios:                                                         │
//! │   ├── traffic (NormalTraffic)                                       │
//! │   ├── security (CredentialStuffing, SqlInjection, PortScan)         │
//! │   ├── performance (MemoryLeak, CpuSpike, InfiniteLoop)              │
//! │   └── distributed (DDoS, CascadeFailure, DataExfiltration, etc.)    │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Design Principles
//!
//! 1. **No Detection Logic** - Simulation only generates logs with ground truth.
//!    Detection/comparison happens in via-bench, not here.
//!
//! 2. **Scenario-Based** - All log generation goes through the Scenario trait.
//!    This allows pluggable, composable anomaly patterns.
//!
//! 3. **Ground Truth Tracking** - Every log knows if it's part of an injected
//!    anomaly. This enables precise benchmarking metrics (precision/recall/F1).
//!
//! 4. **Real-time Ready** - tick() advances simulation time and returns batches.
//!    Can run faster than real-time for batch benchmarking.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use via_sim::{SimulationEngine, scenarios};
//!
//! let mut engine = SimulationEngine::new();
//! engine.start("normal_traffic");
//!
//! // Schedule an anomaly 5 seconds in
//! engine.schedule_anomaly("memory_leak", 5_000_000_000, 10_000_000_000);
//!
//! // Generate logs in 100ms ticks
//! loop {
//!     let batch = engine.tick_ms(100);
//!     // batch.logs contains OTel logs
//!     // batch.ground_truth contains anomaly windows
//!     // batch.metadata has statistics
//! }
//! ```
//!
//! ## Available Scenarios
//!
//! | Category    | Scenario               | Description                           |
//! |-------------|------------------------|---------------------------------------|
//! | Traffic     | `normal_traffic`       | Baseline realistic traffic            |
//! |             | `traffic_spike`        | Sudden traffic burst                  |
//! | Security    | `credential_stuffing`  | Brute force login attempts            |
//! |             | `sql_injection`        | SQL injection probes                  |
//! |             | `port_scan`            | Network port scanning                 |
//! | Performance | `memory_leak`          | Gradual memory increase → OOM         |
//! |             | `cpu_spike`            | High CPU causing timeouts             |
//! |             | `infinite_loop`        | Stack overflow simulation             |
//! | Distributed | `ddos`                 | Multi-source DDoS attack              |
//! |             | `cascade_failure`      | Service failure propagation           |
//! |             | `data_exfiltration`    | Large suspicious data transfers       |
//! |             | `slow_queries`         | Database performance degradation      |
//! |             | `error_spike`          | Sudden error rate increase            |

// Core types - single source of truth
pub mod core;

// Scenarios - pluggable anomaly generators
pub mod scenarios;

// Unified simulation engine
pub mod engine;

// HTTP Control API
pub mod api;

// Re-exports for convenience
pub use core::{
    AnyValue, BatchMetadata, GroundTruth, KeyValue, LogRecord, OTelLog, Resource, ResourceLog,
    ScopeLog, SimulationBatch,
};

pub use engine::{EngineState, EngineStats, SimulationEngine};

pub use scenarios::{
    Scenario,
    create_scenario,
    // Distributed
    distributed::{
        CascadeFailure, DDoSAttack, DataExfiltration, ErrorRateSpike, SlowQueries, TrafficSpike,
    },
    list_scenarios,
    // Performance
    performance::{CpuSpike, InfiniteLoop, MemoryLeak},
    // Security
    security::{CredentialStuffing, PortScan, SqlInjection},
    // Traffic
    traffic::NormalTraffic,
};

pub use api::{
    ApiConfig, ApiResponse, InjectAnomalyRequest, SharedState, SimulationState, StartRequest,
    create_shared_state, handle_change_rate, handle_get_dashboard, handle_get_status,
    handle_inject_anomaly, handle_list_scenarios, handle_pause, handle_resume, handle_start,
    handle_stop, handle_tick, print_api_docs,
};
