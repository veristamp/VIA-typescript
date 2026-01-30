# 🚀 PRD: VIA Tier-1 Engine (Gatekeeper v2)

**System Name:** VIA Gatekeeper
**Tier:** Tier-1 (Hot Path / Firehose)
**Target Throughput:** ≥150,000 events/sec
**Latency Budget:** P99 < 5ms
**Language:** Rust (Pure Native)
**Runtime:** Tokio (Multi-Threaded)
**Deployment Model:** Single Binary (Horizontally Scalable)

---

## 1. Purpose & Scope

The VIA Tier-1 Engine (“Gatekeeper”) is the **front-line ingestion and anomaly detection system** for the VIA platform.

It is responsible for:

* Ingesting high-volume telemetry (logs / metrics / traces)
* Performing **single-pass, streaming, probabilistic anomaly detection**
* Emitting **signals, not truth**
* Buffering raw events and anomalies for downstream Tier-2 systems

### Non-Goals

* No durable storage guarantees
* No global consistency
* No exact analytics
* No blocking I/O in the hot path

**Philosophy:**

> *Fail Open. Fail Fast. Never Stall the Firehose.*

---

## 2. Core Design Principles

1. **Hot Path Purity**

   * No disk I/O
   * No locks on shared mutable state
   * No unbounded allocations

2. **Sharded Actor Model**

   * Deterministic routing of events
   * Single-threaded state mutation per shard
   * Zero contention, zero locks inside state

3. **Bounded Memory**

   * All data structures have fixed or capped size
   * Old state decays or is evicted

4. **Approximate > Accurate**

   * Probabilistic algorithms only
   * Tier-2 is the source of truth

5. **Interface Agnostic**

   * HTTP today
   * Kafka / gRPC / FFI tomorrow
   * Core engine is transport-independent

---

## 3. High-Level Architecture

```
┌─────────────┐
│   Clients   │
└──────┬──────┘
       │
┌──────▼──────────────────────────┐
│ Network + Parsing Layer (Tokio) │  ← parallel on all cores
│ - HTTP (Axum)                   │
│ - JSON / SIMD-JSON              │
└──────┬──────────────────────────┘
       │ LogEntry
       ▼
┌─────────────────────────────────┐
│ Routing Layer (Hash-Based)       │
│ shard_id = hash(user) % N        │
└──────┬──────────────────────────┘
       │
┌──────▼──────────────────────────┐
│ Shard Workers (Actors)           │  ← N single-threaded loops
│ - SafeProfile Map                │
│ - Anomaly Detectors              │
│ - No locks                       │
└──────┬──────────────────────────┘
       │
┌──────▼──────────────────────────┐
│ Async Persistence & Signals      │
│ - Buffered logging               │
│ - Metrics                        │
│ - Alert emission                 │
└─────────────────────────────────┘
```

---

## 4. Ingestion & Interface Layer

### 4.1 Transport

**Primary:** HTTP (Axum)
**Future:** Kafka, gRPC, FFI

### 4.2 Endpoints

#### `POST /ingest`

```json
{
  "u": "user_12345",
  "v": 23.5,
  "t": 1706680000
}
```

* Response: `202 Accepted`
* No synchronous validation beyond schema correctness
* No disk access
* No anomaly blocking

#### `GET /health`

```json
{ "status": "up", "shards": 8 }
```

#### `GET /metrics` (Prometheus)

---

## 5. Parsing & CPU Strategy

### 5.1 Parallel Parsing

* Tokio multi-threaded runtime
* All worker threads:

  * Accept connections
  * Parse payloads
  * Perform hashing
* Parsing cost is **distributed across cores**

### 5.2 JSON Strategy

* Default: `simd-json`
* Fallback: `serde_json`
* Optional binary fast-path:

  * MsgPack
  * Fixed struct binary payload
  * NDJSON batch ingestion

---

## 6. Routing & Sharding Model (Critical)

### 6.1 Deterministic Sharding

```text
shard_id = xxhash64(user_id) & (N - 1)
```

* N is a power of two
* Guarantees all events for a user land on the same shard

### 6.2 Why This Matters

* Prevents **state smearing**
* Ensures accurate:

  * IAT calculations
  * Burst detection
  * Per-user distributions

---

## 7. Shard Worker Model (Actor System)

Each shard is a **single-threaded event loop**:

* Receives events via `mpsc::channel`
* Owns its own `HashMap<u64, SafeProfile>`
* No locks
* No atomics
* No shared mutable state

### Worker Responsibilities

* Update SafeProfile
* Run anomaly detectors
* Emit anomaly signals
* Apply decay / eviction

---

## 8. SafeProfile (Per-Entity State)

```rust
pub struct SafeProfile {
    pub last_seen_ts: u64,

    // Holt-Winters
    pub hw_level: f32,
    pub hw_trend: f32,
    pub hw_seasonality: [f32; 4],

    // HyperLogLog
    pub hll_registers: [u8; 64],

    // Fading Histogram
    pub hist_buckets: [u32; 10],
    pub hist_decay_ts: u64,
}
```

* No heap allocation during updates
* Fixed-size memory footprint
* Safe for millions of profiles

---

## 9. Anomaly Detection Engine

### 9.1 Detector Architecture

Each event passes through an **ensemble of detectors**:

```
Event
  ├─ Volume Detector (HW)
  ├─ Cardinality Velocity Detector (HLL)
  ├─ Distribution Detector (Histogram / SimHash)
  └─ Burst Detector (EWMA + CUSUM)
        ↓
Weighted Voting → Severity Score
```

---

### 9.2 Detectors

#### 1. Volume Detector (Holt-Winters)

* Input: `1 / IAT` (clamped)
* Output: Expected rate
* Trigger: `observed > predicted + kσ`

#### 2. Cardinality Velocity (HLL)

* Tracks **rate of new unique users**
* Trigger: Sudden slope change (botnets, credential stuffing)

#### 3. Distribution Detector

* Numeric: Fading histogram
* Text: SimHash
* Trigger: Rare bucket burst

#### 4. Burst Detector

* EWMA baseline on IAT
* CUSUM drift detection
* Trigger: Tight clustering

---

## 10. Persistence & Backpressure

### 10.1 Async Logging

* `tokio::mpsc::channel (bounded)`
* Dedicated writer task
* `BufWriter<File>`
* Hourly rotation

### 10.2 Backpressure Policy

* If channel full:

  * Drop event
  * Increment `via_dropped_total`
* **Never block ingestion**

---

## 11. Metrics & Observability

### Core Metrics

* `via_ingest_total`
* `via_anomalies_total`
* `via_dropped_total`
* `via_active_profiles`
* `via_channel_depth`

### Logging

* Structured logs (`tracing`)
* Anomalies always logged
* Raw logs sampled

---

## 12. Simulation / Red Team Module

### Purpose

* Stress testing
* Regression testing
* Attack modeling

### Capabilities

* Poisson traffic
* LogNormal latency
* Credential stuffing
* SQL injection
* Port scanning
* Resource exhaustion

### Execution Modes

* Direct engine injection (bypass HTTP)
* Full stack via localhost

---

## 13. Failure Modes & Guarantees

| Scenario        | Behavior             |
| --------------- | -------------------- |
| CPU Saturation  | Shed load            |
| Channel Full    | Drop logs            |
| Disk Failure    | Continue ingestion   |
| Detector Panic  | Isolated shard crash |
| Memory Pressure | Evict cold profiles  |

---

## 14. Deployment & Scaling

* Single binary
* Horizontal scale via replicas
* Stateless externally
* Shards scale with CPU cores

---

## 15. Success Criteria

* ≥150k events/sec sustained
* Stable RSS over time
* Zero global locks
* No GC pauses
* Deterministic anomaly detection per entity

---

## 16. Future Extensions (Non-Blocking)

* Kafka ingestion
* WASM-based detectors
* SIMD histogram updates
* GPU offload for batch analytics


