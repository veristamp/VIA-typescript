# VIA-Core Tier-1: SOTA Anomaly Detection - Implementation Plan

**Date:** 2026-02-01
**Status:** ✅ COMPLETE
**Goal:** Transform Tier-1 into a production-ready, explainable, adaptive anomaly detection engine

---

## Implementation Summary

All core modules have been implemented and tested:

| Module | Status | Tests |
|--------|--------|-------|
| `signal.rs` | ✅ Complete | 3/3 passing |
| `feedback.rs` | ✅ Complete | 3/3 passing |
| `registry.rs` | ✅ Complete | 4/4 passing |
| `checkpoint.rs` | ✅ Complete | 2/2 passing |
| `engine.rs` | ✅ Refactored | 5/5 passing |
| `lib.rs` | ✅ Updated | 3/3 passing |
| `gatekeeper.rs` | ✅ Updated | Compiles |

### Key Deliverables:
1. **Rich `AnomalySignal`** - Full detector breakdown with SHAP-like attribution
2. **Feedback Loop** - Thread-safe channel for Tier-2 → Tier-1 learning
3. **LRU ProfileRegistry** - Memory-bounded with 100K profiles per shard
4. **Checkpoint Protocol** - Serialization for Bun-managed persistence
5. **Two-Stage Pipeline** - Detection (10 detectors) → Decision (AdaptiveEnsemble)
6. **Comprehensive FFI** - Signal accessors, feedback input, checkpoint export/import

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              VIA-CORE TIER-1                                     │
│                                                                                   │
│  ┌──────────────┐     ┌────────────────────────────────────────────────────┐    │
│  │   Ingest     │     │                 DETECTION PIPELINE                  │    │
│  │   (Axum)     │────▶│  ┌─────────────┐    ┌─────────────┐    ┌────────┐  │    │
│  │              │     │  │  Detector   │    │  Adaptive   │    │ Signal │  │    │
│  │  SIMD-JSON   │     │  │  Orchestra  │───▶│  Ensemble   │───▶│ Emitter│  │────│───▶ TO TIER-2
│  │  + Hashing   │     │  │  (10 det.)  │    │  (Thompson) │    │        │  │    │
│  └──────────────┘     │  └─────────────┘    └──────┬──────┘    └────────┘  │    │
│                       │                            │                        │    │
│                       │                    ┌───────▼───────┐               │    │
│  ┌──────────────┐     │                    │   Explainer   │               │    │
│  │   Profile    │◀────│────────────────────│  (Attribution)│               │    │
│  │   Registry   │     │                    └───────────────┘               │    │
│  │   (LRU)      │     └────────────────────────────────────────────────────┘    │
│  └──────┬───────┘                                                                │
│         │                                                                        │
│         ▼                                                                        │
│  ┌──────────────┐     ┌──────────────┐                                          │
│  │  Checkpoint  │────▶│  TO TIER-2   │  (Bun manages persistence)               │
│  │  Serializer  │     │  via Channel │                                          │
│  └──────────────┘     └──────────────┘                                          │
│                                                                                   │
│  ◀─────────────────── FEEDBACK CHANNEL ◀───────────────────────────────────────│
│  (Tier-2 sends: entity_hash, was_true_positive, detector_scores)                │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Core Data Structures (Files to Create/Modify)

### 1.1 New File: `src/signal.rs`
Rich signal output for Tier-2 consumption.

```rust
// Key structures:
- AnomalySignal        // Full output to Tier-2
- DetectorScore        // Per-detector score (fixed array, no alloc)
- BaselineSummary      // Entity behavioral context
- Severity             // Enum: None, Low, Medium, High, Critical
- SignalType           // Which detector(s) triggered
```

### 1.2 New File: `src/feedback.rs`
Feedback loop from Tier-2.

```rust
// Key structures:
- FeedbackEvent        // Tier-2 → Tier-1 confirmation
- FeedbackChannel      // Thread-safe channel for receiving feedback
```

### 1.3 New File: `src/registry.rs`
Memory-bounded profile registry with LRU eviction.

```rust
// Key structures:
- ProfileRegistry      // LRU-bounded HashMap
- ProfileEntry         // Wrapper with metadata (last_access, priority)
- EvictionPolicy       // LRU, TTL, or Priority-based
```

### 1.4 Modified File: `src/engine.rs`
Refactor to use new architecture.

```rust
// Changes:
- AnomalyProfile uses AdaptiveEnsemble
- Two-stage pipeline: Detection → Decision
- Rich output: AnomalySignal instead of AnomalyResult
- Feedback integration for weight learning
```

### 1.5 New File: `src/checkpoint.rs`
Serialization for Bun-managed persistence.

```rust
// Key structures:
- CheckpointData       // Serialized profile state
- CheckpointRequest    // Sent to Tier-2 for storage
- RecoveryRequest      // Received from Tier-2 on startup
```

---

## Phase 2: Implementation Steps

### Step 1: Create `signal.rs` (Rich Output)
- Define `AnomalySignal` with full detector breakdown
- Use `#[repr(C)]` for FFI compatibility
- Fixed-size arrays for zero allocation
- Include attribution (primary/secondary contributors)

### Step 2: Create `feedback.rs` (Learning Loop)
- Define `FeedbackEvent` structure
- Implement thread-safe channel
- Wire feedback to AdaptiveEnsemble

### Step 3: Create `registry.rs` (Memory Management)
- Implement LRU cache with configurable size
- Track access times and evict stale profiles
- Memory pressure callbacks

### Step 4: Refactor `engine.rs` (Core Pipeline)
- Integrate AdaptiveEnsemble for weight learning
- Two-stage detection (collect signals → combine with ensemble)
- Generate rich AnomalySignal output
- Process feedback events

### Step 5: Create `checkpoint.rs` (Persistence)
- Serialize detector states (HoltWinters, HLL, etc.)
- Compact binary format (bincode + lz4)
- Recovery logic for startup

### Step 6: Update FFI (`lib.rs`)
- New FFI functions for rich signals
- Feedback input function
- Checkpoint export/import

### Step 7: Update Gatekeeper (`gatekeeper.rs`)
- Use ProfileRegistry instead of HashMap
- Add feedback endpoint
- Add checkpoint endpoint

---

## Phase 3: File Structure After Implementation

```
via-core/crates/via-core/src/
├── algo/                      # Unchanged (detectors)
│   ├── adaptive_ensemble.rs   # Already exists, will be wired in
│   ├── adaptive_threshold.rs
│   ├── behavioral_fingerprint.rs
│   ├── ... (other detectors)
│   └── mod.rs
├── engine.rs                  # REFACTORED: Two-stage pipeline
├── signal.rs                  # NEW: Rich output structures
├── feedback.rs                # NEW: Learning loop
├── registry.rs                # NEW: LRU profile management
├── checkpoint.rs              # NEW: Persistence protocol
├── gatekeeper.rs              # UPDATED: New endpoints
└── lib.rs                     # UPDATED: New FFI interface
```

---

## Implementation Order

| Order | File | Action | Blocking |
|-------|------|--------|----------|
| 1 | `signal.rs` | Create | None |
| 2 | `feedback.rs` | Create | None |
| 3 | `registry.rs` | Create | None |
| 4 | `algo/mod.rs` | Update exports | None |
| 5 | `engine.rs` | Major refactor | Steps 1-4 |
| 6 | `checkpoint.rs` | Create | Step 5 |
| 7 | `lib.rs` | Update FFI | Step 5 |
| 8 | `gatekeeper.rs` | Update server | Step 5-7 |

---

## Key Design Decisions

### 1. Zero-Allocation Hot Path
- Fixed-size arrays: `[DetectorScore; 10]`
- Pre-allocated buffers
- Stack-allocated signal structs

### 2. Feedback Learning
- Thompson Sampling (already in adaptive_ensemble.rs)
- Async feedback processing (doesn't block hot path)
- Batched weight updates

### 3. Memory Bounds
- Default: 100,000 profiles max
- LRU eviction when limit reached
- Configurable via TOML (future)

### 4. Checkpoint Protocol
- Triggered by Tier-2 request (not timer-based)
- Tier-2 stores the binary blob
- On startup, Tier-2 sends last checkpoint via FFI

---

## Success Criteria

- [ ] All 10 detectors produce independent signals
- [ ] AdaptiveEnsemble combines with learned weights
- [ ] Rich AnomalySignal output with attribution
- [ ] Feedback loop updates ensemble weights
- [ ] LRU eviction keeps memory bounded
- [ ] Checkpoint/recovery eliminates warmup
- [ ] Throughput stays ≥500K EPS (50% of current is acceptable for accuracy gains)

---

**BEGIN IMPLEMENTATION**
