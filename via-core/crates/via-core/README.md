# via-core

High-performance anomaly detection engine for real-time streaming data.

## Architecture

```
via-core/
├── lib.rs              # FFI bindings & core exports
├── engine.rs           # Main detection engine (profile management)
├── gatekeeper.rs       # HTTP server for ingestion (axum-based)
├── signal.rs           # Anomaly signal generation & attribution
├── feedback.rs         # Online learning from corrections
├── checkpoint.rs       # State persistence & recovery
├── registry.rs         # LRU profile cache with eviction
└── algo/               # Detection algorithms
    ├── adaptive_ensemble.rs    # Multi-armed bandit ensemble
    ├── adaptive_threshold.rs   # EWMA/Percentile/MAD thresholds
    ├── behavioral_fingerprint.rs # Entity profiling
    ├── drift_detector.rs       # ADWIN, Page-Hinkley, KL-divergence
    ├── enhanced_cusum.rs       # CUSUM with V-mask & FIR
    ├── multi_scale.rs          # Temporal decomposition
    ├── rrcf.rs                 # Robust Random Cut Forest
    ├── spectral_residual.rs    # FFT-based detection
    ├── ewma.rs                 # Exponential weighted moving average
    ├── histogram.rs            # Streaming histogram
    ├── hll.rs                  # HyperLogLog cardinality
    └── holtwinters.rs          # Triple exponential smoothing
```

## Algorithms

### Detectors

| Algorithm | Use Case | Complexity |
|-----------|----------|------------|
| **RRCF** | Multivariate anomalies | O(log n) |
| **Spectral Residual** | Time-series spikes | O(n log n) |
| **Enhanced CUSUM** | Mean shifts | O(1) |
| **Drift Detector** | Distribution changes | O(1) |
| **Multi-Scale** | Seasonal decomposition | O(n) |
| **Adaptive Ensemble** | Combines detectors | O(k) |
| **Behavioral Fingerprint** | Entity profiling | O(1) |

### Primitives

| Module | Purpose |
|--------|---------|
| `ewma` | Exponential weighted moving average |
| `histogram` | Streaming histogram |
| `hll` | HyperLogLog cardinality |
| `holtwinters` | Triple exponential smoothing |
| `adaptive_threshold` | Dynamic threshold (EWMA/Percentile/MAD) |

## Usage

### As Library

```rust
use via_core::{ViaProfile, engine::Engine};

let mut engine = Engine::new();
let profile = engine.get_or_create_profile(entity_id);
let (score, is_anomaly) = profile.update(value, timestamp_ns);
```

### As HTTP Server

```bash
cargo run --bin gatekeeper -- --port 8080
```

```bash
curl -X POST http://localhost:8080/ingest \
  -H "Content-Type: application/json" \
  -d '{"entity_id": 123, "value": 100.0, "timestamp_ns": 1706745600000000000}'
```

## Build

```bash
# Library
cargo build --release

# With C bindings (cdylib)
cargo build --release --lib

# Gatekeeper server
cargo build --release --bin gatekeeper
```

## Performance

- **Throughput**: 500K+ events/sec per core
- **Latency**: <1μs per update (hot path)
- **Memory**: ~1KB per entity profile

## Tests

```bash
cargo test --lib
# 66 tests, 0 failures
```

## License

