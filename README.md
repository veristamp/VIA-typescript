# VIA: Multi-Tier Anomaly Detection Platform

<p align="center">
  <img src="https://img.shields.io/badge/Tier-1 Rust-brightgreen" alt="Tier-1 Rust">
  <img src="https://img.shields.io/badge/Tier-2 TypeScript-blue" alt="Tier-2 TypeScript">
  <img src="https://img.shields.io/badge/Throughput-30k%2B%20EPS-orange" alt="30k+ EPS">
  <img src="https://img.shields.io/badge/License-MIT-blue" alt="License MIT">
</p>

**VIA** (VeriStamp Incident Atlas) is an enterprise-grade, multi-tier anomaly detection platform that combines high-throughput Tier-1 detection with intelligent Tier-2 correlation and incident management. Built for real-time security monitoring, operational resilience, and automated incident response.

---

## Architecture Overview

VIA employs a **two-tier detection architecture** designed for scalability and accuracy:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           EVENT SOURCES                                  │
│                    (Logs, Metrics, Audit Trails)                         │
└─────────────────────────────────┬───────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  TIER-1: Gatekeeper (via-core)                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Sharded Actor Model (lock-free)                                │   │
│  │ • 10 Detector Ensemble (Burst, Spectral, Trend, RRCF, etc.)    │   │
│  │ • Static Dispatch for sub-30μs latency                          │   │
│  │ • OTel & JSON ingestion                                        │   │
│  │ • Forwards anomaly signals to Tier-2                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────┬───────────────────────────────────────┘
                                  │ Anomaly Signals
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  TIER-2: Correlation Engine (Bun + TypeScript)                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Qdrant vector similarity clustering                           │   │
│  │ • Temporal, Semantic, Trace correlation                        │   │
│  │ • Incident deduplication & escalation                           │   │
│  │ • PostgreSQL persistence                                        │   │
│  │ • Feedback loop to Tier-1 for model tuning                     │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Key Features

- **10-Layer Detection Ensemble**: Burst, Spectral/FFT, Trend/CUSUM, RRCF, Cardinality/HLL, Behavioral Fingerprint, ADWIN/Page-Hinkley, Multi-Scale, Adaptive Histograms, UCB/Thompson Sampling
- **30k+ Events Per Second** processing capacity
- **Sub-30μs latency** per event detection
- **OTel-native** log format support
- **Self-learning** with feedback loops for precision tuning
- **Deterministic simulation** via via-sim for reproducible benchmarks

---

## Prerequisites

| Component | Version | Purpose |
|----------|---------|---------|
| Rust | 1.70+ | Tier-1 engine compilation |
| Bun | 1.17+ | Tier-2 runtime |
| PostgreSQL | 14+ | Persistent storage |
| Qdrant | 1.7+ | Vector similarity search |
| Node.js | 20+ | Build tools |

---

## Quick Start

### 1. Database Setup

```bash
# Start PostgreSQL and Qdrant (using Docker)
docker run -d --name postgres-via -e POSTGRES_USER=via -e POSTGRES_PASSWORD=via -e POSTGRES_DB=via_registry -p 5432:5432 postgres:14
docker run -d --name qdrant-via -p 6333:6333 qdrant/qdrant:v1.7.0

# Run migrations
cd VIA-typescript
bun run db:migrate
```

### 2. Start Tier-2 Backend

```bash
# Terminal 1: Start Tier-2
cd VIA-typescript
bun run src/main.ts

# Health check
curl http://127.0.0.1:3000/health
```

### 3. Start Tier-1 Gatekeeper

```bash
# Terminal 2: Start Gatekeeper
cd VIA-typescript/via-core
GATEKEEPER_ADDR=0.0.0.0:3001 TIER2_URL=http://127.0.0.1:3000 \
  cargo run --release -p via-core --bin gatekeeper

# Health check  
curl http://127.0.0.1:3001/health
```

---

## Running Benchmarks

### End-to-End Pipeline Benchmark

The recommended way to evaluate the full system using realistic OTel data:

```bash
# Quick validation (1 minute)
cd VIA-typescript/via-core
./target/release/via-bench pipeline --scenario quick --duration 1

# Mixed workload (5 minutes, multiple anomaly types)
./target/release/via-bench pipeline --scenario mixed --duration 5

# Security audit
./target/release/via-bench pipeline --scenario security --duration 3

# Performance stress test
./target/release/via-bench pipeline --scenario performance --duration 2
```

### Tier-1 Only Benchmark

```bash
# Run built-in benchmark scenarios
./target/release/via-bench run-all -v

# Specific scenario
./target/release/via-bench mixed-workload --duration 2
```

---

## API Endpoints

### Tier-1 Gatekeeper (Port 3001)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/ingest` | Single event ingestion |
| POST | `/ingest/batch` | Batch event ingestion |
| POST | `/ingest/otel` | OTel log format ingestion |
| POST | `/feedback` | Submit feedback for model tuning |
| GET | `/stats` | System statistics |
| GET | `/health` | Health check |
| GET | `/metrics` | Prometheus metrics |

**Example: OTel Ingestion**
```bash
curl -X POST http://127.0.0.1:3001/ingest/otel \
  -H "Content-Type: application/json" \
  -d '{
    "resourceLogs": [{
      "resource": {"attributes": []},
      "scopeLogs": [{
        "logRecords": [{
          "timeUnixNano": "1709846400000000000",
          "traceId": "abc123",
          "spanId": "def456",
          "severityNumber": 9,
          "severityText": "INFO",
          "body": {"stringValue": "Request processed"},
          "attributes": [
            {"key": "service.name", "value": {"stringValue": "api-gateway"}},
            {"key": "http.duration_ms", "value": {"doubleValue": 150.5}}
          ]
        }]
      }]
    }]
  }'
```

### Tier-2 Backend (Port 3000)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/analysis/incidents` | List incidents |
| GET | `/analysis/incidents/run/:runId` | Incidents for a specific run |
| GET | `/analysis/pipeline/stats` | Queue statistics |
| POST | `/tier2/anomalies` | Receive signals from Tier-1 |

---

## Available Scenarios (via-sim)

The benchmark uses realistic anomaly scenarios from `via-sim`:

| Scenario | Category | Description |
|----------|----------|-------------|
| `credential_stuffing` | Security | Brute force login attempts |
| `ddos` | Distributed | DDoS attack from multiple IPs |
| `memory_leak` | Performance | Gradual memory consumption |
| `slow_queries` | Distributed | Database performance degradation |
| `traffic_spike` | Distributed | Sudden traffic burst |

Run benchmark:
```bash
cd via-core
./target/release/via-bench pipeline --scenario mixed --duration 6
```

---

## Benchmark Results

Recent pipeline benchmark results (6-minute mixed workload with 5 anomaly types):

| Metric | Value |
|--------|-------|
| **Throughput** | ~1,778 EPS |
| **Detection Precision** | 44.3% |
| **Detection Recall** | 39.8% |
| **Incident Precision** | 72.7% |
| **Incident Recall** | 88.9% |
| **Incident F1** | 0.80 |
| **P50 Latency** | 31μs |
| **P95 Latency** | 63μs |

### Per-Anomaly Detection

| Anomaly Type | Category | Ground Truth | Detected | Recall |
|--------------|----------|-------------|----------|--------|
| Credential Stuffing | Security | 3,000 | 2,899 | 96.6% |
| DDoS | Distributed | 30,000 | 4,030 | 13.4% |
| Memory Leak | Performance | 208 | 208 | 100% |
| Slow Queries | Distributed | 450 | 449 | 99.8% |
| Traffic Spike | Distributed | 30,000 | 17,773 | 59.2% |

Recent pipeline benchmark results (5-minute mixed workload):

| Metric | Value |
|--------|-------|
| **Throughput** | ~1,300 EPS |
| **Detection Precision** | 44.5% |
| **Detection Recall** | 63.3% |
| **Incident Precision** | 80% |
| **Incident Recall** | 100% |
| **Incident F1** | 0.89 |
| **P50 Latency** | 25μs |
| **P95 Latency** | 43μs |

### Per-Anomaly Detection

| Anomaly Type | Ground Truth | Detected | Recall |
|--------------|-------------|----------|--------|
| Credential Stuffing | 3,000 | 2,900 | 96.7% |
| Memory Leak | 208 | 208 | 100% |
| Traffic Spike | 30,000 | 17,920 | 59.7% |

---

## Project Structure

```
VIA-typescript/
├── src/                      # Tier-2 (Bun/TypeScript)
│   ├── api/                  # HTTP routes
│   ├── services/             # Business logic
│   ├── db/                   # Database schema
│   └── modules/tier2/        # Tier-2 domain models
├── via-core/                 # Tier-1 (Rust)
│   ├── crates/
│   │   ├── via-core/         # Detection engine
│   │   ├── via-bench/        # Benchmark suite
│   │   └── via-sim/          # Deterministic simulator
│   └── target/release/       # Compiled binaries
├── scripts/                  # Utility scripts
└── docs/                    # Architecture docs
```

---

## Configuration

### Environment Variables

**Tier-1 (Gatekeeper):**
- `GATEKEEPER_ADDR`: Server bind address (default: `0.0.0.0:3001`)
- `TIER2_URL`: Tier-2 endpoint for signal forwarding

**Tier-2:**
- `DATABASE_URL`: PostgreSQL connection string
- `QDRANT_URL`: Qdrant HTTP endpoint

---

## License

MIT License - See LICENSE file for details.

---

## Built With

- **Tier-1**: Rust, simd-json, tokio, axum
- **Tier-2**: Bun, Hono, Drizzle ORM, PostgreSQL, Qdrant
- **Benchmarking**: Custom via-sim deterministic simulation engine
