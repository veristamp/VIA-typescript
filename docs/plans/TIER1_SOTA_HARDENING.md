# VIA Tier-1 SOTA Hardening Spec

Status: Implemented in current workspace revision.

## Scope

This spec applies to Tier-1 only:
- `via-core/crates/via-core/src/engine.rs`
- `via-core/crates/via-core/src/gatekeeper.rs`
- `via-core/crates/via-core/src/algo/adaptive_ensemble.rs`
- `via-core/crates/via-core/src/registry.rs`

## Phase 1: Correctness

1. Ensemble combiner must return only `(ensemble_score, confidence)`.
2. Detector weights must be read from ensemble state, not inferred from per-detector score vectors.
3. Feedback routing must be deterministic by shard ownership:
   - `shard_id = entity_hash % shard_count`.
4. Checkpoint restore must recover:
   - Ensemble weights
   - Bandit alpha/beta parameters
   - Sample/update counter

## Phase 2: Runtime Resilience

1. Backpressure must remain fail-open and observable.
2. Drop reasons must be split with dedicated counters:
   - ingest queue
   - shard queue
   - persistence queue
   - feedback queue
3. Feedback profile misses must be tracked.
4. Persistence I/O failures must not crash the process loop; they must log and continue.
5. Server boot/shutdown must avoid panic-only control flow in runtime paths.

## Phase 3: Adaptive Detection Quality

Decision policy is hybrid:
1. Detector trigger floor
2. Ensemble score floor
3. Adaptive ensemble threshold with confidence gating

All values are configured in `ProfileConfig`:
- `min_detector_score_for_anomaly`
- `min_ensemble_score_for_anomaly`
- `use_adaptive_ensemble_threshold`
- `confidence_threshold`

## Phase 4: Contract and Compatibility

1. Tier-1 output contract has explicit `schema_version`.
2. `/stats` exposes both service version and signal schema version.
3. Tier-1 startup must not be coupled to optional simulation symbol presence.

## Operational Rules

1. Tier-1 never blocks ingestion on downstream persistence.
2. Tier-1 emits signals, not final truth.
3. Tier-2 feedback is eventually consistent and best-effort.
4. Tier-1 should prefer bounded queues and deterministic loss accounting over unbounded memory growth.

