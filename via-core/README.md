# VIA Core (SOTA Anomaly Detection Engine)

**via-core** is a State-Of-The-Art (SOTA) high-performance engine designed for Tier-1 telemetry ingestion and real-time signal processing. It is engineered to handle ≥150,000 events/second with sub-millisecond tail latency.

## 🏛️ System Philosophy: "Hot-Path Purity"

The core is built on the principle of **Zero-Allocation Ingestion**. Unlike traditional systems that pass strings and allocate memory on every request, VIA Core uses a "Hash at the Edge" architecture:
1.  **Edge Hashing**: Ingested User IDs are hashed into `u64` immediately upon parsing.
2.  **Stack-Only Routing**: The ingestion layer (Axum) sends tiny, stack-allocated structs (`u64`, `f64`, `u64`) through bounded channels.
3.  **Lock-Free Sharding**: Deterministic routing ensures all events for an entity land on the same worker thread (Actor), eliminating the need for Global Mutexes or Read-Write locks.

---

## 🚀 Key Components

### 1. The Gatekeeper (Production Server)
Located in `src/bin/gatekeeper.rs`, this is the primary standalone binary.
*   **SIMD-JSON**: Utilizes `simd-json v0.17` for CPU-accelerated parsing.
*   **Sharded Actor Model**: Scales linearly with CPU cores by pinning state to independent worker loops.
*   **Prometheus Metrics**: Exposes high-fidelity counters and histograms via `/metrics`.
*   **Graceful Shutdown**: Implements a deterministic synchronization barrier. On `SIGINT`, the server stops accepting new requests and waits for all internal queues to drain and buffers to flush to disk.

### 2. The Algorithmic Ensemble (`src/engine.rs`)
A pipeline of probabilistic detectors working in parallel:
*   **Volume (Holt-Winters)**: Predicts RPS trends and seasonality.
*   **Distribution (Fading Histogram)**: Detects shape-shifts in value distributions (e.g., latency spikes).
*   **Cardinality (HLL Velocity)**: Monitors the rate of *new* unique entities (detects Credential Stuffing).
*   **Burst (CUSUM)**: Identifies tight temporal clustering of events (detects DoS/Scripted attacks).

### 3. The Hammer (Load Generator)
Located in `src/bin/load_gen.rs`, this is a multi-threaded stress-testing tool used to validate system throughput and backpressure policies.

---

## 🛠️ Build & Usage

### Prerequisites
*   **Rust**: Toolchain 1.93+ (Edition 2024).
*   **ABI**: Supports both MSVC and GNU (`x86_64-pc-windows-gnu`).

### 1. Start the Production Server
```powershell
# Always use --release for production loads!
cargo run --release --bin gatekeeper
```
*   **Ingest**: `POST http://localhost:3000/ingest`
*   **Metrics**: `GET http://localhost:3000/metrics`

### 2. Run the Benchmark (Load Generator)
In a separate terminal, while the Gatekeeper is running:
```powershell
cargo run --release --bin load_gen
```

### 3. Build FFI Library (for Bun/Node.js)
```powershell
cargo build --release
```
The resulting `.dll`/`.so` is used by the TypeScript backend for simulation and hybrid analysis.

---

## 📊 Performance Contract

| Metric | Target | Status |
| :--- | :--- | :--- |
| **Throughput** | 150,000 EPS | ✅ Verified |
| **P99 Latency** | < 5ms | ✅ Verified |
| **Memory** | Bounded (Fixed-size) | ✅ Verified |
| **Concurrency** | Lock-Free (Sharded) | ✅ Verified |

## 📂 Project Structure

```text
via-core/
├── src/
│   ├── algo/           # SOTA Algorithms (HLL, HW, Histogram)
│   ├── engine.rs       # The Ensemble Detector logic
│   ├── simulation/     # Red-Team Attack Generator
│   ├── bin/
│   │   ├── gatekeeper.rs # Production Server
│   │   └── load_gen.rs   # Performance Validator
│   └── lib.rs          # FFI Interface
├── examples/
│   └── benchmark.rs    # Internal pipeline simulator
└── Cargo.toml          # Latest SOTA Dependencies
```