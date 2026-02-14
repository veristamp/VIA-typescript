# Tier-1 ↔ Tier-2 Integration Fix

## Changes Made

### 1. New Forwarder Module (`forwarder.rs`)

Created HTTP forwarding layer to send anomaly signals from Tier-1 (Rust) to Tier-2 (Bun):

```rust
pub struct Tier2Forwarder {
    tx: mpsc::Sender<AnomalySignal>,
    stats: Arc<ForwarderStats>,
}
```

**Features:**
- Async batching with configurable batch size and flush interval
- Retry with exponential backoff
- Bounded channel with backpressure
- Statistics tracking (sent, failed, retried, dropped, batches)

### 2. Updated Gatekeeper (`gatekeeper.rs`)

**Added:**
- `forwarder: Option<Arc<Tier2Forwarder>>` to `AppState`
- Environment variable `TIER2_URL` to enable forwarding
- Forwarding in `ShardWorker.run()` when anomalies detected

**Usage:**
```bash
# Enable Tier-2 forwarding
export TIER2_URL=http://localhost:3000
./gatekeeper
```

### 3. Policy Enhancements (`policy.rs`)

**Added to `PolicySnapshot`:**
- `canary_percent: f64` - for gradual rollout
- `fallback_version: Option<String>` - for safe rollback

**Added to `PatternRule`:**
- `detector_priors: Option<Vec<DetectorPriorAdjustment>>` - detector weight adjustments

**New struct:**
```rust
pub struct DetectorPriorAdjustment {
    pub detector_id: u8,
    pub alpha_delta: f64,
    pub beta_delta: f64,
}
```

## Alignment with Blueprint

| Blueprint Requirement | Status |
|----------------------|--------|
| `label_class` enum | ✅ Already implemented |
| `pattern_id` optional | ✅ Already implemented |
| `review_source` | ✅ Already implemented |
| `feedback_latency_ms` | ✅ Already implemented |
| `canary_percent` | ✅ Added |
| `fallback_version` | ✅ Added |
| Detector prior adjustments | ✅ Added |
| Policy checksum in checkpoint | ✅ Already implemented |
| Tier-1 → Tier-2 forwarding | ✅ Added |

## Data Flow

```
┌──────────────────────────────────────────────────────────────────┐
│                         TIER-1 (Rust)                            │
│  ┌──────────┐    ┌──────────┐    ┌──────────────┐                │
│  │ Ingest   │───▶│ Detector │───▶│ AnomalySignal│                │
│  │ API      │    │ Engine   │    │              │                │
│  └──────────┘    └──────────┘    └──────┬───────┘                │
│                                          │                        │
│                    ┌─────────────────────┼─────────────────────┐  │
│                    ▼                     ▼                     ▼  │
│            ┌──────────────┐    ┌──────────────┐    ┌──────────┐ │
│            │ JSONL Files  │    │Tier2Forwarder│    │ Feedback │ │
│            │ (fallback)   │    │   (HTTP)     │    │ Channel  │ │
│            └──────────────┘    └──────┬───────┘    └──────────┘ │
└───────────────────────────────────────┼─────────────────────────┘
                                        │
                                        ▼
┌──────────────────────────────────────────────────────────────────┐
│                         TIER-2 (Bun)                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
│  │ /tier2/      │───▶│ Tier2Queue   │───▶│ Incident     │        │
│  │ anomalies    │    │ Service      │    │ Service      │        │
│  └──────────────┘    └──────────────┘    └──────────────┘        │
│                              │                                   │
│                              ▼                                   │
│                    ┌──────────────────┐                          │
│                    │ Qdrant + Postgres│                          │
│                    └──────────────────┘                          │
└──────────────────────────────────────────────────────────────────┘
```

## Configuration

### Tier-1 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TIER2_URL` | (disabled) | Tier-2 base URL for forwarding |

### Forwarder Config

```rust
ForwarderConfig {
    tier2_url: "http://localhost:3000",
    batch_size: 100,
    flush_interval_ms: 1000,
    max_retries: 3,
    retry_base_delay_ms: 100,
    channel_capacity: 10_000,
    timeout_ms: 5000,
}
```

## Testing

Run the full test suite:
```bash
cargo test --lib -p via-core
```

Build release:
```bash
cargo build --release -p via-core
```
