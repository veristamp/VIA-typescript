# VIA Core (Rust Engine)

**via-core** is the high-performance heart of the VIA anomaly detection system. It is a Rust crate that compiles into a dynamic library (`.dll`, `.so`, `.dylib`) to be loaded by the Bun/Node.js runtime via FFI (Foreign Function Interface).

This engine is designed for **Tier-1 Anomaly Detection**: extremely fast, streaming, single-pass analysis of high-volume telemetry data (logs, metrics, traces).

## 🚀 Key Features

*   **Zero-Copy Ingestion**: Processes events via FFI pointers without serialization overhead where possible.
*   **Streaming Algorithms**: No heavy databases. All state is maintained in-memory using compact probabilistic data structures.
*   **Production Simulation**: Includes a built-in OTel-compliant log generator for stress testing and red-teaming.

## 🧠 Algorithmic Core (`src/algo`)

The engine combines multiple statistical signals to detect anomalies in real-time.

### 1. Holt-Winters (Triple Exponential Smoothing)
*   **File**: `algo/holtwinters.rs`
*   **Purpose**: Tracks **Volume Trends** (Requests Per Second).
*   **Logic**: Decomposes the signal into **Level**, **Trend**, and **Seasonality**. It learns the "normal" heartbeat of your traffic and flags deviations (e.g., unexpected surges or drops).
*   **Adaptation**: We feed `1 / Inter-Arrival-Time` into HW to estimate instantaneous rate on a per-event basis.

### 2. Fading Histogram
*   **File**: `algo/histogram.rs`
*   **Purpose**: Tracks **Value Distributions** (e.g., Latency, Payload Size).
*   **Logic**: Maintains a weighted histogram where old data exponentially decays (`decay` factor). New data is bucketed.
*   **Detection**: If a new value falls into a "rare" bucket (low probability mass), it is flagged as a distribution anomaly (e.g., P99 latency becoming P50).

### 3. HyperLogLog (HLL)
*   **File**: `algo/hll.rs`
*   **Purpose**: Tracks **Cardinality** (Unique Users, IP addresses).
*   **Logic**: Uses hashing and bit-counting (probabilistic) to estimate the count of unique items with constant memory usage (~12KB), regardless of finding 1 million or 1 billion users.
*   **Detection**: Sudden spikes in cardinality (e.g., Credential Stuffing / Botnets).

### 4. EWMA & CUSUM
*   **File**: `algo/ewma.rs`, `algo/cusum.rs`
*   **Purpose**: Fast-reaction burst detection.
*   **Logic**: Monitors the Inter-Arrival Time (IAT) of events. A CUSUM (Cumulative Sum) drift detector triggers if the IAT consistently drops below the baseline.

## 🎮 Simulation Engine (`src/simulation`)

A built-in "Red Team" engine that generates OTel-compliant JSON logs.

*   **Traffic**: Poisson-process arrivals with LogNormal latency distributions.
*   **Attacks**:
    *   **Credential Stuffing**: High-volume, rotating IP login failures.
    *   **SQL Injection**: Malicious payloads in DB logs.
    *   **Port Scans**: Rapid multi-port connection attempts.
*   **Resources**: Memory leaks and CPU spikes.

## 🛠️ Build & Usage

### Prerequisites
*   **Rust Toolchain**: `rustup`, `cargo`
*   **Target**: `x86_64-pc-windows-gnu` (if avoiding MSVC) or standard MSVC.

### Build Commands

```powershell
# 1. Standard Debug Build (Fast Compilation)
cargo build

# 2. Release Build (Max Performance - Use for Prod)
cargo build --release

# 3. Build for Specific Target (e.g., GNU ABI)
cargo build --target x86_64-pc-windows-gnu
```

### Running Benchmarks
We have a live ingestion benchmark that simulates ~130k events/sec.

```powershell
cargo run --example benchmark
```

## 🔌 FFI Interface (`src/lib.rs`)

The library exposes a C-ABI compatible interface for Bun:

*   `create_profile(...)`: Allocates a new detector.
*   `process_event(...)`: Feeds a single event (timestamp, user_id, value).
*   `simulation_tick(...)`: Advances the simulation and returns a batch of logs.
*   `free_...`: Memory cleanup (Manual memory management required on JS side!).

**Note**: All pointers returned by `create_*` must be freed using their respective `free_*` functions to avoid memory leaks in the host application.
