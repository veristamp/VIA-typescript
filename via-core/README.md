# 🛡️ VIA-Core: State-of-the-Art Adaptive Anomaly Detection Engine

[![Build Status](https://img.shields.io/badge/Build-Passing-brightgreen)](https://github.com/srimon12/via-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Performance: 30k+ EPS](https://img.shields.io/badge/Performance-30k+--EPS-orange)](BENCHMARK.md)
[![FFI Support](https://img.shields.io/badge/FFI-C%2FC%2B%2B-blueviolet)](src/lib.rs)

**VIA-Core** is a high-performance, sharded, and ultra-low-latency anomaly detection engine written in Rust. It is designed to act as a **Tier-1 Security Gatekeeper**, providing real-time behavioral analysis for massive event streams (logs, metrics, audit trails) with sub-millisecond precision.

Leveraging **Static Dispatch**, **Zero-Allocation SIMD JSON parsing**, and an **Actor-based Sharding Architecture**, VIA-Core can process over **1.8 Million events per minute** on a single node while maintaining a microscopic memory footprint.

---

## 🚀 Key Performance Metrics

Based on the latest unified benchmarks in `BENCHMARK.md`:

*   **Peak Throughput:** Over **31,700 Events Per Second (EPS)**.
*   **Average Latency:** **28.0 µs** (microseconds) per event (single-threaded).
*   **Tail Latency (P99):** **38.0 µs**.
*   **Architecture:** Lock-free, Actor-based sharding with zero-copy internals.
*   **Memory Efficiency:** LRU-bounded profile registry capable of tracking millions of unique entities.

---

## 🏗️ Architecture: The "Ferrari" Design

VIA-Core is built on three fundamental pillars of high-performance systems engineering:

### 1. Decoupled Ingestion Layer
The `gatekeeper` server utilizes an asynchronous ingestion strategy. Incoming HTTP/REST requests are parsed using **simd-json** and immediately pushed into a high-capacity `tokio::mpsc` channel. This decouples the network I/O from the compute-intensive detection logic, enabling the engine to "drink from a firehose" without blocking callers.

### 2. Sharded Actor Model
To eliminate lock contention (mutex/rwlock bottlenecks), VIA-Core shards the identity space. Every unique entity (`UID`) is hashed and routed to a specific **Shard Worker**. 
*   Each shard owns its memory exclusively.
*   Processing is 100% lock-free within the hot path.
*   Linear scaling with CPU core count.

### 3. Static Dispatch Engine
Unlike traditional OO-based engines that use `Box<dyn Detector>` (vtable lookups), VIA-Core utilizes **Static Dispatch**. All 10+ detectors are hard-compiled into the `AnomalyProfile` struct. This allows the LLVM compiler to inline detection logic, providing a **25-30% performance boost** over dynamic dispatch alternatives.

---

## 🧠 Detection Stack: 10 Layer Defense

VIA-Core employs a "Defense in Depth" strategy using a heterogeneous ensemble of algorithms:

| Layer | Algorithm | Detection Focus |
| :--- | :--- | :--- |
| **Burst** | **Dynamic-IAT CUSUM** | Sudden clustering of events (DDoS, Bot-bursts). |
| **Temporal** | **Spectral Residual (FFT)** | Saliency detection in time-series (Spikes). |
| **Trend** | **Enhanced CUSUM** | Sustained shifts in mean behavior (Exfiltration). |
| **Isolation** | **Random Cut Forest (RRCF)** | High-dimensional point outliers. |
| **Velocity** | **Cardinality (HLL)** | Velocity changes and unique ID expansion. |
| **Context** | **Behavioral Fingerprint** | Deviation from historical time/day norms. |
| **Continuity** | **ADWIN / Page-Hinkley** | Gradual concept drift and trend changes. |
| **Scale** | **Multi-Scale temporal** | Anomalies occurring at varying resolutions. |
| **Distribution** | **Adaptive Histograms** | Shifts in value distributions (Payload shifts). |
| **Adaptive** | **UCB/Thompson Sampling** | Real-time weight adjustment based on feedback. |

---

## 🛠️ Components & File Structure

### `crates/via-core/src/`
*   **`engine.rs`**: The heart of the system. Contains `AnomalyProfile` and the static dispatch logic.
*   **`gatekeeper.rs`**: The production-ready ingestion server (Axum + SIMD-JSON).
*   **`registry.rs`**: A memory-bounded LRU cache for managing millions of entity profiles.
*   **`signal.rs`**: The "Ground Truth" output format, containing attribution, scores, and confidence.
*   **`algo/`**: Individual implementations of world-class detection algorithms.
*   **`lib.rs`**: The FFI boundary, exposing the engine to C, C++, and Python.

### `crates/via-bench/`
A comprehensive benchmarking suite that simulates Mixed Workloads, Security Audits, and Pure CPU Stress Tests.

---

## � Getting Started

### Prerequisites
*   Rust 1.70+ (Stable)
*   LLVM / Clang (for SIMD optimizations)
*   **PowerShell/Bash**

### Installation
Clone the repository and build in release mode for maximum performance:

```bash
git clone https://github.com/srimon12/via-bun-via-core.git
cd via-bun/via-core
cargo build --release
```

### Running the Gatekeeper (Production Ingest)
The gatekeeper is the primary entry point for live data:

```bash
cargo run --release -p via-core --bin gatekeeper
```
*   **Ingest:** `POST /ingest` (Single) or `POST /ingest/batch` (Batching supported).
*   **Metrics:** `GET /metrics` (Prometheus format).
*   **Health:** `GET /health`.

### Running Benchmarks
To verify performance on your hardware:

```bash
# Full benchmark suite with batching
cargo run --release -p via-bench --bin via-bench -- run-all -v -b 500

# Targeted throughput test
cargo run --release -p via-bench --bin via-bench -- throughput -d 1 -b 500
```

---

## 🔌 Ingestion API

VIA-Core supports high-throughput JSON batching.

**Endpoint:** `POST /ingest/batch`
**Payload:**
```json
[
  { "u": "user_123", "v": 124.5, "t": 1706828400000000 },
  { "u": "user_456", "v": 10.2, "t": 1706828400000005 }
]
```

**Output (Anomaly Signal):**
If an anomaly is detected, the system generates a rich signal:
```json
{
  "entity_hash": 123456789,
  "ensemble_score": 0.82,
  "severity": "Critical",
  "attribution": {
    "primary": "Spectral/FFT",
    "reason": "Sudden frequency spike detected"
  },
  "confidence": 0.94
}
```

---

## 🛡️ Self-Learning & Feedback Loop

VIA-Core is not just a bunch of hardcoded rules. It features an **Adaptive Ensemble** that learns from your environment.

### Tier-2 Feedback
When a security analyst or an LLM reviews an anomaly, they can send feedback back to the engine:

```bash
POST /feedback
{
  "entity_hash": 12345678,
  "was_true_positive": true,
  "confidence": 1.0,
  "source": "human"
}
```

The engine uses **Multi-Armed Bandit (MAB)** logic (UCB1) to adjust the weights of individual detectors. If `Burst` is too noisy in your network, the system will automatically "de-weight" it over time, prioritizing detectors like `Spectral` or `RRCF` that provide higher precision.

---

## 🔧 Advanced Configuration

You can tune the engine via `ProfileConfig` in `engine.rs`:

*   **Warmup Period:** `50` events (Adjust depending on ingestion speed).
*   **Ensemble Threshold:** `0.10` (Lower for higher recall, higher for precision).
*   **Shard Count:** Automatically matches your CPU hardware threads.
*   **Registry Size:** Defaults to `100,000` profiles per shard.

---

## 🧪 Testing

VIA-Core maintains a 100% pass rate on its internal algorithm unit tests.

```bash
# Run all core tests
cargo test -p via-core

# Run algorithmic specific tests
cargo test algo::spectral_residual
```

---

## 📜 Roadmap & Future Enhancements

*   [x] **Decoupled Ingestion Layer** (Completed)
*   [x] **Dynamic IAT Baseline Learning** (Completed)
*   [ ] **NATS JetStream Integration** (Planned)
*   [ ] **GPU-Accelerated RRCF** (Researching)
*   [ ] **WebAssembly (WASM) Edge Deployment** (In Progress)

---

## 📄 License

VIA-Core is licensed under the **MIT License**. See `LICENSE` for details.

---

## 🤝 Contributing

Contributions are welcome! Please ensure you run `cargo fmt` and `cargo clippy` before submitting a PR. Performance regressions are strictly checked; any PR affecting the hot path must include a `via-bench` report showing zero impact on P50 latency.

**Maintainer:** Srimon12 
**Built by:** The VIA Team

---

> *"Real-time is relative. In VIA-Core, real-time is 28 microseconds."*