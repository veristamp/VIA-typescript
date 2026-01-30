# VIA Core (High-Performance Engine)

**via-core** is the algorithmic heart of the VIA anomaly detection platform. It is a dual-mode Rust crate that serves two critical functions:
1.  **FFI Library**: A dynamic library (`.dll`/`.so`) loaded by the Bun/Node.js backend for simulation and hybrid logic.
2.  **Gatekeeper Server**: A standalone, high-throughput Tier-1 ingestion server capable of processing >150,000 events/second.

This system is engineered for **Zero-Copy Performance**, **Lock-Free Concurrency**, and **Probabilistic Analytics**.

---

## 🏛️ System Architecture

### 1. The Gatekeeper (Tier-1 Engine)
The Gatekeeper (`src/bin/gatekeeper.rs`) is the production entry point. It implements a **Sharded Actor Model** to handle massive data streams without global locks.

*   **Network Layer**: `Tokio` + `Axum` for async I/O, utilizing `simd-json` for ultra-fast payload parsing.
*   **Routing**: Deterministic Sharding (`xxHash`) guarantees that all events for a specific user/entity are routed to the same worker thread.
*   **Workers**: 8+ independent threads (Actors) that manage state in `HashMap`s. No mutexes or atomics are used in the hot path.
*   **Persistence**: Dedicated async thread for anomaly logging with automatic hourly file rotation.
*   **Metrics**: Prometheus endpoint (`/metrics`) exposing lock-free counters for ingestion, drops, and latency.

### 2. The Algorithmic Core (`src/engine.rs`)
The detection logic is built on a **Component-Based Signal Architecture**. Every event is analyzed by an ensemble of detectors, each outputting a weighted score.

| Detector | Signal | Implementation | Logic |
| :--- | :--- | :--- | :--- |
| **Volume** | Request Rate (RPS) | `Holt-Winters` + `EWMA` | Smoothes inter-arrival times to predict baseline RPS trends and seasonality. Flags deviations. |
| **Distribution** | Value Shape (Latency) | `FadingHistogram` | Maintains an exponentially decaying histogram to detect "tail" events (e.g., P99 latency spikes). |
| **Cardinality** | User Growth | `HyperLogLog` + `Velocity` | Tracks the *rate of new unique users*. Sudden slope changes indicate attacks (e.g., Credential Stuffing). |
| **Burst** | Micro-Timing | `CUSUM` | Monitors inter-arrival timing for tight clustering indicative of scripted attacks or DoS. |

### 3. Simulation Engine (`src/simulation`)
A built-in "Red Team" generator used to validate the engine. It produces OTel-compliant logs with realistic anomalies.
*   **Scenarios**: Normal Traffic, Memory Leaks, CPU Spikes, SQL Injection, Port Scanning.
*   **Control**: Exposed via FFI to allow the UI to dynamically inject attacks into the stream.

---

## 🛠️ Build & Usage

### Prerequisites
*   **Rust**: Stable toolchain (Edition 2024 supported).
*   **OS**: Windows (MSVC/GNU), Linux, macOS.

### 1. Building the Gatekeeper (Server)
Run this to start the production ingestion server.
```powershell
# Build & Run (Release mode is CRITICAL for performance)
cargo run --release --bin gatekeeper
```

**Endpoints:**
*   `POST /ingest`: Accepts JSON `{"u": "user", "v": 12.5, "t": 1234567890}`
*   `GET /metrics`: Prometheus metrics.
*   `GET /health`: Health check.

### 2. Building the Library (FFI for Bun)
Run this to build the `.dll` for the Node.js/Bun backend.
```powershell
cargo build --release
```
Artifact location: `target/release/via_core.dll`

### 3. Running Benchmarks
Validate the internal engine throughput (expected: ~150k events/sec).
```powershell
cargo run --release --example benchmark
```

---

## 📊 Performance Characteristics

*   **Throughput**: Linear scaling with CPU cores. ~150k EPS on 8 cores.
*   **Latency**: P99 processing time < 1ms (internal).
*   **Memory**: Bounded per-profile state (Probabilistic structures).
*   **Backpressure**: "Fail Open" policy. If queues fill up, events are dropped to preserve system stability.

## 📂 Project Structure

```text
via-core/
├── src/
│   ├── algo/           # Core algorithms (HLL, HW, EWMA)
│   ├── engine.rs       # The SOTA Ensemble Logic
│   ├── simulation/     # Attack Generator & Scenarios
│   ├── bin/
│   │   └── gatekeeper.rs # The Production Server (Axum/Tokio)
│   └── lib.rs          # FFI Exports for Bun
├── examples/
│   └── benchmark.rs    # Ingestion Pipeline Simulator
└── Cargo.toml          # Dependencies
```
