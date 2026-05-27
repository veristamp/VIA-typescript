# Tier-1 / Tier-2 Repair Handoff

Status: current workspace handoff, not a finished architecture.

## Assumptions

1. We are intentionally moving toward Rust-native Tier-1/Tier-2 behavior, but not by blindly deleting the analysis service before the Rust side can own the behavior correctly.
2. No backward compatibility is required for dead HTTP ingest paths or old Tier-2 simulation routes.
3. Qdrant is enabled for now.
4. Embeddings are hash vectors for now: `EMBEDDING_MODE=hash`, `EMBEDDING_DIMENSION=64`.
5. We should not build protobufs during normal Cargo builds. Generated code is checked in.
6. Release-build benchmark numbers matter more than dev-build numbers.

## Current Branch State

This branch contains a large in-progress migration. Treat it as a repair branch, not a tiny patch.

Implemented in the current workspace:

1. Tier-1 anomaly ingest no longer uses the old HTTP stream route.
2. Tier-1 sends anomaly batches to Tier-2 over gRPC/Connect.
3. The old `src/api/routes/stream.ts` path is deleted.
4. Tier-2 exposes `via.tier2.v1.Tier2Service/SubmitAnomalyBatch`.
5. TypeScript generated protobuf files are checked in under `src/gen/via/tier2/v1/`.
6. Rust protobuf/client files are checked in under `via-core/crates/via-core/src/pb/`.
7. `via-core/crates/via-core/build.rs` was removed so protos are not generated at Cargo build time.
8. Tier-1-side signal normalization and dedupe started moving into Rust.
9. The tiny TypeScript incident decision module was deleted and moved into Rust as `via-core/crates/via-core/src/tier2.rs`.
10. Tier-2 Qdrant defaults now favor hash embeddings instead of accidental external embedding assumptions.
11. Tier-2 correlation now preserves incident evidence arrays on upsert instead of replacing them.
12. The Python e2e script accepts env-driven config, seed control, hash embedding mode, and gRPC Tier-2 ingest.

Important caveat: anomaly ingest is migrated, but the full Tier-1/Tier-2 control plane is not fully gRPC yet. `Tier1SyncService` still uses HTTP-style endpoints for feedback, policy snapshot, and incident decision sync. That is still technical debt if the rule is "no HTTP between tiers."

## Why We Are Not Done

The transport layer was not the real source of the quality problem. HTTP did add overhead, but the bigger problem is that Tier-1 is not currently behaving like a calibrated ten-detector ensemble.

The uncomfortable read from the benchmark is:

1. ChangePoint/Trend carries most of the useful detection.
2. Burst/IAT contributes meaningful recall, but with more noise.
3. Cardinality/Velocity contributes some signal.
4. Drift/Concept, RRCF/Isolation, Volume/RPS, Distribution/Value, Spectral/FFT, MultiScale/Temporal, and Behavioral/Fingerprint are either weak, scenario-mismatched, or noisy in the quick benchmark.
5. Good-looking previous numbers came partly from loose thresholds and OR-gating weak detector outputs, not from all detectors proving useful.

This means "10 algos" currently looks better as a feature list than as a reliable decision system.

## Benchmark Evidence

Tier-1-only quick benchmark, release build:

```bash
cargo run --release -p via-bench --bin via-bench -- \
  --seed 20260527 \
  -o /tmp/via-tier1-quick.json \
  quick
```

Observed:

| Metric | Value |
| --- | ---: |
| Precision | 0.6301545474 |
| Recall | 0.5898666667 |
| F1 | 0.6093454082 |
| True positives | 8848 |
| False positives | 5193 |
| False negatives | 6152 |
| Total detections | 14041 |

Per-detector quick benchmark summary:

| Detector | Precision | Recall | F1 | Notes |
| --- | ---: | ---: | ---: | --- |
| ChangePoint/Trend | 0.8549682875 | 0.8088 | 0.83124 | Main useful detector |
| Burst/IAT | 0.6060 | 0.49947 | 0.5476 | Useful but noisy |
| Cardinality/Velocity | 0.61033 | 0.1772 | 0.27466 | Partial support |
| Drift/Concept | 0.93844 | 0.04573 | 0.0872 | High precision, almost no recall |
| RRCF/Isolation | 0.48653 | 0.03853 | 0.0714 | Weak contribution |
| Volume/RPS | 0.00466 | 0.0004 | 0.000736 | Mostly noise here |
| Distribution/Value | 0.01168 | 0.00033 | 0.000648 | Mostly noise here |
| Spectral/FFT | 0.5 | 0.0002 | 0.000399 | Barely triggers |
| MultiScale/Temporal | 0 | 0 | 0 | No useful signal here |
| Behavioral/Fingerprint | 0 | 0 | 0 | No trigger in quick run |

Release pipeline benchmark with Tier-2 ingest:

```bash
target/release/via-bench \
  --seed 20260527 \
  -o /tmp/via-tier2-pipeline-quick-fixed4.json \
  pipeline \
  --scenario quick \
  --duration 1 \
  --send-batch 256 \
  --tier2-url http://127.0.0.1:3000 \
  --tier2-grpc-url http://127.0.0.1:3002
```

Observed:

| Metric | Value |
| --- | ---: |
| Detection precision | 0.6305646423 |
| Detection recall | 0.5911333333 |
| Detection F1 | 0.6102126488 |
| Incident precision | 0.5 |
| Incident recall | 1.0 |
| Incident F1 | 0.6666666667 |
| Split error rate | 0.0 |
| Tier-2 events sent | 14062 |

Interpretation: in the apples-to-apples `via-bench` path, Tier-2 did not reduce Tier-1 detection F1. The detection F1 stayed around 0.61. The remaining bad behavior is that Tier-2 created one real temporal incident and one false temporal incident from a Tier-1 false-positive detector tail.

The Python e2e script produced a much worse number:

```bash
uv run --with requests scripts/e2e-eval.py \
  --duration 120 \
  --entities 8 \
  --batch-size 200 \
  --verbose \
  --seed 20260527
```

Observed:

| Metric | Value |
| --- | ---: |
| Sent | 960 |
| Forwarded | 677 |
| Forward failed | 0 |
| Incidents | 11 |
| Precision | 70.00% |
| Recall | 12.50% |
| F1 | 0.2121 |

Interpretation: this script is useful as a smoke/e2e tool, but it is not apples-to-apples with `via-bench`. It uses a different randomized generator and a weaker incident scoring model. Do not use it alone to declare Tier-1 quality regression.

## Tier-1 Root Problems

### 1. The Decision Gate Is Too Loose

`via-core/crates/via-core/src/engine.rs` currently behaves like a loose OR gate:

1. Any detector score above `min_detector_score_for_anomaly` can mark the signal anomalous.
2. The ensemble floor can mark the signal anomalous.
3. The adaptive threshold can mark the signal anomalous.

Current defaults include:

```rust
min_detector_score_for_anomaly: 0.10
min_ensemble_score_for_anomaly: 0.10
confidence_threshold: 0.5
use_adaptive_ensemble_threshold: true
```

That means one weak detector can create a final anomaly. This is not a principled ensemble.

### 2. Ensemble Score Is Diluted, Then Bypassed

`AdaptiveEnsemble::combine()` includes silent detectors in the total weight. That dilutes the ensemble score when only one or two detectors fire.

Then `engine.rs` bypasses that dilution with `any_detector_fired`. The result is internally inconsistent:

1. `is_anomaly = true`
2. ensemble `score` can remain low
3. `severity` can remain `None`
4. Tier-2 receives a "real anomaly" with weak score/severity semantics

This is how Tier-2 gets poisoned by weak Tier-1 tails.

### 3. Severity Is Derived From The Wrong Thing

Severity is derived from the adjusted ensemble score. If the ensemble score is diluted by silent detectors, a real single-detector event can have low severity. If a weak detector trips the OR gate, a bad event can still pass as an anomaly.

Severity should be derived from accepted evidence, not only from diluted ensemble score.

### 4. Adaptive Ensemble Is Not Actually Learning In The Quick Benchmark

`AdaptiveEnsemble` learns from feedback. The Tier-1-only benchmark does not appear to feed ground-truth feedback into Thompson sampling during the run. So the "adaptive" part is mostly static in the benchmark.

If we benchmark adaptiveness, the benchmark must feed labels. If we do not feed labels, we should not claim adaptive quality from that number.

### 5. Benchmark Path Is Not Production-Equivalent Enough

The Tier-1-only quick benchmark uses a single `AnomalyProfile` over the simulated stream. The gatekeeper production path uses profile registry/entity hashing behavior. That means the Tier-1-only benchmark can look better or cleaner than the real deployed path.

We need a gatekeeper-shaped benchmark before we trust any number.

## Tier-2 Root Problems

### 1. Temporal-Only Correlation Is Too Weak

With 64-dim hash vectors and limited semantic context, Tier-2 can group:

1. same trace
2. same rhythm hash
3. same temporal bucket
4. approximate vector similarity

But it still cannot deeply understand what happened. A temporal bucket full of Tier-1 false positives can become a false incident.

Temporal-only candidates should require stronger corroboration before incident creation.

### 2. Hash Embeddings Are Fine For Migration, Not Enough For Threat Semantics

Hash vectors unblock the Rust migration and avoid external embeddings, but they are mostly behavioral fingerprints. They do not understand:

1. log body content
2. request path semantics
3. stack traces
4. attack-class similarity
5. multi-step chains

The architecture should allow richer context strings and later ONNX/local embeddings without rewriting the transport.

### 3. Tier-2 Should Not Hide Tier-1 Quality Problems

Tier-2 can correlate and compile policy. It should not be forced to rescue weak Tier-1 final decisions. If Tier-1 emits noisy "final anomalies," Tier-2 will either:

1. create false incidents, or
2. add stricter filters and miss real incidents.

The correct repair starts in Tier-1.

## Repair Plan

### Phase 1: Make The Benchmark Honest

1. Add a gatekeeper-shaped release benchmark that uses the same profile registry/entity path as production.
2. Keep the Tier-1-only benchmark, but label it as an engine benchmark rather than a production-equivalent benchmark.
3. Add per-detector precision, recall, F1, trigger count, average score, and contribution-to-final-decision reporting.
4. Add scenario-level reporting so one happy path cannot hide detector failure.
5. Feed ground-truth labels into the adaptive ensemble when evaluating adaptiveness.

Verification:

```bash
cargo run --release -p via-bench --bin via-bench -- --seed 20260527 quick
cargo run --release -p via-bench --bin via-bench -- --seed 20260527 pipeline --scenario quick
```

### Phase 2: Fix Tier-1 Decision Semantics

1. Separate detector activity from final anomaly decision:
   - `detector_fired`
   - `candidate_evidence`
   - `final_anomaly`
2. Replace the loose OR gate with evidence classes:
   - strong single-detector evidence can trigger alone
   - weak detector evidence requires corroboration
   - detector-tail evidence cannot create standalone final anomalies
3. Calibrate thresholds per detector. A score of `0.10` does not mean the same thing for every algorithm.
4. Derive severity from accepted evidence, not only from diluted ensemble score.
5. Emit enough evidence metadata for Tier-2 to understand why Tier-1 accepted the event.

Verification:

```bash
cargo test --workspace -- --test-threads=1
cargo run --release -p via-bench --bin via-bench -- --seed 20260527 -o /tmp/via-tier1-repaired.json quick
```

### Phase 3: Disable Or Demote Bad Detectors Until They Prove Value

Do not keep detectors in the final decision path just because they exist.

Initial candidates to demote or require corroboration:

1. Volume/RPS
2. Distribution/Value
3. Spectral/FFT
4. MultiScale/Temporal
5. Behavioral/Fingerprint
6. RRCF/Isolation unless calibrated better

They can still run for telemetry, but they should not create final anomalies without proof.

Verification:

```bash
cargo run --release -p via-bench --bin via-bench -- --seed 20260527 -o /tmp/via-detectors.json quick
```

Review per-detector reports before promoting any detector back into final decisioning.

### Phase 4: Finish The gRPC Boundary

1. Keep anomaly ingest on gRPC/Connect.
2. Move feedback sync to gRPC.
3. Move policy snapshot sync to gRPC.
4. Move incident decision sync to gRPC or remove it if Rust owns the decision locally.
5. Delete the old HTTP-only tier bridge paths once replaced.

Do not reintroduce Fastify just to host Connect. The current direction uses Connect over Node HTTP/2.

Verification:

```bash
rg "fetch\\(|http://|/feedback|/policy|/incident/decision" src via-core/crates/via-core/src
bunx tsc --noEmit
cargo test --workspace -- --test-threads=1
```

### Phase 5: Make Tier-2 Correlation Stricter

1. Do not create incidents from temporal-only weak evidence.
2. Require trace/rhythm/vector/entity corroboration, or require accepted strong Tier-1 evidence.
3. Preserve member evidence arrays on incident upsert.
4. Keep hash embeddings now, but enrich the context payload so real embeddings can be dropped in later.
5. Avoid hardcoded benchmark matching. Rules must be scenario-agnostic.

Verification:

```bash
uv run --with requests scripts/e2e-eval.py --duration 120 --entities 8 --batch-size 200 --seed 20260527
target/release/via-bench --seed 20260527 -o /tmp/via-tier2-pipeline.json pipeline --scenario quick --duration 1 --send-batch 256 --tier2-grpc-url http://127.0.0.1:3002
```

## Files To Start With

Tier-1 decisioning:

1. `via-core/crates/via-core/src/engine.rs`
2. `via-core/crates/via-core/src/algo/adaptive_ensemble.rs`
3. `via-core/crates/via-core/src/signal.rs`
4. `via-core/crates/via-core/src/tier2.rs`

Benchmarks:

1. `via-core/crates/via-bench/src/main.rs`
2. `via-core/crates/via-bench/src/pipeline.rs`
3. `scripts/e2e-eval.py`

Tier-2 ingest/correlation:

1. `proto/via/tier2/v1/tier2.proto`
2. `src/rpc/tier2-rpc.ts`
3. `src/services/forensic-analysis-service.ts`
4. `src/services/incident-service.ts`
5. `src/services/qdrant-service.ts`
6. `src/services/tier1-sync-service.ts`

Config:

1. `.env.example`
2. `src/config/settings.ts`
3. `package.json`

## Minimum Done Criteria For The Next Repair Pass

1. Release-build Tier-1 benchmark improves for the right reason, not by hiding false positives in the scorer.
2. Per-detector reports show which detectors are useful and which are demoted.
3. Tier-1 final anomaly decisions no longer come from a weak single-detector OR gate.
4. `is_anomaly`, `score`, and `severity` are semantically consistent.
5. Tier-2 no longer creates incidents from weak temporal-only detector tails.
6. Anomaly ingest and control-plane sync no longer rely on ad hoc HTTP bridges between tiers.
7. `uv run --with requests scripts/e2e-eval.py ...` and release `via-bench` both run cleanly.
8. Any remaining hardcoded simulation knobs are either removed or explicitly scoped to benchmarks.

