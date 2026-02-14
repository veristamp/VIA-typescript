# VIA Architecture Upgrade Blueprint (Tier-1 + Tier-2)

This document proposes a full upgrade path for a high-scale, high-accuracy anomaly pipeline where:
- Tier-1 (Rust) is always-on, low-latency, state-consistent.
- Tier-2 (Bun + Qdrant + Postgres) learns patterns and compiles runtime policy back to Tier-1.
- CPU reduces after adaptation without sacrificing recall by using uncertainty routing, not blind detector skipping.

## 1) Target Architecture

Tier-1 should always update detector state for every event, then choose decision complexity based on uncertainty and policy match.

Tier-2 should stop being only a suppression/patch sink and become a policy compiler:
- mine repeated patterns,
- validate with confidence and recency,
- emit policy artifacts with TTL/version/rollback metadata,
- feed compact rules back to Tier-1.

## 2) Current Gaps (by file)

### Tier-1 (Rust)
- `via-core/crates/via-core/src/engine.rs`
  - Heavy detectors and combine path are always fully evaluated in hot path.
  - No uncertainty-gated score compute path yet.
- `via-core/crates/via-core/src/gatekeeper.rs`
  - Backpressure currently drops queue traffic with `try_send` under pressure.
  - No policy ingestion endpoint or policy version state.
- `via-core/crates/via-core/src/feedback.rs`
  - Feedback shape is mostly TP/FP/FN + score vector; no class/novelty/pattern metadata.
- `via-core/crates/via-core/src/checkpoint.rs`
  - No persisted policy snapshot/version for deterministic restart.

### Tier-2 (Bun)
- `src/services/control-service.ts`
  - Control is binary (`suppressionCache`, `patchRegistry`) with no weighted policy output.
- `src/services/incident-service.ts`
  - Decides incident status, but does not compile/emit Tier-1 runtime policy.
- `src/services/tier2-queue-service.ts`
  - Single in-flight flush loop; adaptation latency and throughput will bottleneck at scale.
- `src/services/qdrant-service.ts`
  - Unbounded per-batch embedding fanout via `Promise.all`; no concurrency limiter.
  - Collection strategy is good, but no policy feature extraction pipeline.
- `src/db/schema.ts`, `src/db/registry.ts`
  - Missing explicit tables and APIs for versioned policy artifacts and rollout states.
- `src/api/routes/control.ts`
  - Exposes suppress/patch actions only; no policy compile/publish workflow.
- `src/api/routes/stream.ts`
  - Receives Tier-1 anomalies, but no channel for policy acks/version negotiation.

## 3) Upgrade Plan (Phased)

## Phase A: Contract and Model Upgrade

### A1. Add policy artifact schema (Tier-2 DB)
Update:
- `src/db/schema.ts`
- `src/db/registry.ts`

Add tables:
- `tier1_policy_artifacts`
  - `policy_version`, `created_at`, `status`, `compiled_json`, `feature_flags`, `rollback_of`
- `tier1_policy_metrics`
  - `policy_version`, `window_start_ts`, `window_end_ts`, `precision`, `recall`, `latency_p95`, `latency_p99`, `drop_rate`

### A2. Enrich feedback protocol (Tier-1 <-> Tier-2)
Update:
- `via-core/crates/via-core/src/feedback.rs`
- `via-core/crates/via-core/src/gatekeeper.rs`
- `src/services/tier2-service.ts`
- `src/types.ts`

Add fields:
- `label_class`: `benign_known | attack_known | novel | uncertain`
- `pattern_id` (optional)
- `review_source` detail (`human`, `llm`, `auto`)
- `feedback_latency_ms`

Reason:
- Tier-1 should learn class-conditional priors, not only global TP/FP.

## Phase B: Tier-2 Policy Compiler

### B1. Create policy compiler service
Add:
- `src/services/policy-compiler-service.ts` (new)

Inputs:
- incident graph, decisions, historical metrics, patch/suppress controls.

Outputs:
- compiled policy artifact:
  - detector prior adjustments per pattern/class
  - threshold deltas
  - TTLs and safety bounds
  - rollout metadata (`canary_percent`, `fallback_version`)

### B2. Wire compiler into incident decision flow
Update:
- `src/services/incident-service.ts`
- `src/services/control-service.ts`
- `src/services/evaluation-service.ts`

Change:
- `ControlService` becomes policy registry + state API (not only binary suppression).
- `IncidentService.applyCandidates` triggers policy candidate generation.
- `EvaluationService` produces policy fitness windows used for auto-promote/rollback.

### B3. Add API for policy publish and fetch
Add/update:
- `src/api/routes/control.ts`

New endpoints:
- `POST /control/policy/compile`
- `POST /control/policy/publish`
- `POST /control/policy/rollback`
- `GET /control/policy/current`
- `GET /control/policy/:version`

## Phase C: Tier-1 Policy Runtime and Decision Hook

### C1. Add policy runtime module in Rust
Add:
- `via-core/crates/via-core/src/policy.rs` (new)
- export via `via-core/crates/via-core/src/lib.rs`

Core structs:
- `PolicySnapshot`
- `PatternRule`
- `DetectorAdjustment`
- `PolicyRuntime` (lock-free read path, atomic snapshot swap)

### C2. Hook policy runtime into decision path
Update:
- `via-core/crates/via-core/src/engine.rs`

Flow:
1. run detector state updates as today (all detectors),
2. derive fast pattern key from event + detector sketch,
3. if policy match with high confidence:
   - apply detector weight/threshold adjustments,
   - run cheap combine path,
4. if no match or high uncertainty:
   - run full combine path.

Important:
- No detector state skip.
- Only score/decision complexity changes.

### C3. Add policy control endpoint to Gatekeeper
Update:
- `via-core/crates/via-core/src/gatekeeper.rs`

New endpoints:
- `POST /policy/snapshot` (push compiled policy)
- `GET /policy/version`
- `POST /policy/rollback`

Also include version in `/stats` response for observability.

### C4. Persist policy with checkpoints
Update:
- `via-core/crates/via-core/src/checkpoint.rs`

Persist:
- active policy version
- policy checksum
- optional snapshot metadata

## Phase D: Throughput and Latency Hardening

### D1. Tier-2 queue concurrency and backoff
Update:
- `src/services/tier2-queue-service.ts`

Changes:
- configurable worker concurrency (`N` in-flight flush workers),
- bounded retry with jittered exponential backoff,
- age-based prioritization to avoid starvation.

### D2. Embedding/Qdrant concurrency controls
Update:
- `src/services/qdrant-service.ts`

Changes:
- add semaphore/limiter for embedding requests,
- bounded parallel upserts per collection,
- retry policy per operation class.

### D3. Tier-1 backpressure policy (loss-aware)
Update:
- `via-core/crates/via-core/src/gatekeeper.rs`

Changes:
- classify drops by reason and severity,
- degrade path for low-risk traffic before outright drop,
- keep feedback/control channel priority higher than ingest.

## Phase E: Measurement and Safe Rollout

### E1. Online quality/latency metrics
Update:
- `via-core/crates/via-core/src/gatekeeper.rs`
- `src/services/evaluation-service.ts`

Track per policy version:
- precision/recall proxy,
- novelty detection rate,
- p95/p99 latency,
- queue lag,
- drop rate.

### E2. Canary + rollback automation
Update:
- `src/services/policy-compiler-service.ts` (new)
- `src/services/control-service.ts`
- `src/api/routes/control.ts`

Logic:
- publish new policy to canary shard set,
- compare metrics window to baseline,
- auto-promote or rollback.

## 4) Practical Priority Order

1. Phase A (contracts + schema)  
2. Phase C1/C2 (Tier-1 policy runtime + hook)  
3. Phase B (compiler + publish flow)  
4. Phase D (queue and embedding throughput hardening)  
5. Phase E (canary and automated rollback)

## 5) Non-Negotiable Design Rules

- Never skip detector state updates in Tier-1.
- Keep Tier-1 decision path deterministic for same policy version + input.
- Every policy must have TTL + rollback target.
- Every feedback item should carry confidence and class.
- Every rollout must be measurable by versioned metrics.

## 6) Suggested New Files

- `via-core/crates/via-core/src/policy.rs`
- `src/services/policy-compiler-service.ts`
- `src/api/routes/policy.ts` (optional split from control routes)

## 7) Immediate Next Implementation Slice (small but high impact)

1. Add `PolicySnapshot` and `PolicyRuntime` in Rust (`policy.rs`).  
2. Add policy apply hook in `engine.rs` before final anomaly decision.  
3. Add `/policy/snapshot` endpoint in `gatekeeper.rs`.  
4. Add Tier-2 policy artifact table and publish endpoint (`schema.ts`, `registry.ts`, `control.ts`).  
5. Add policy version to Tier-1 `/stats` and Tier-2 incident decisions.

## 8) Tier-2 Embedding and Qdrant Speed Plan (Accuracy-Preserving)

This section focuses on reducing Tier-2 latency/cost while preserving forensic accuracy.

### 8.1 Immediate bottlenecks to fix

#### A) Per-event embedding fanout in ingestion
Update:
- `src/services/qdrant-service.ts`

Current issue:
- `ingestToTier2` computes dense embeddings per event and uses broad `Promise.all`, which creates request storms under load.

Fix:
- Add batched embedding API calls (`input: string[]`) with chunking (for example 64/128 texts).
- Add bounded concurrency limiter for embedding calls.
- Add bounded concurrency limiter for Qdrant upserts per collection.

#### B) Single in-flight queue flush
Update:
- `src/services/tier2-queue-service.ts`

Current issue:
- Queue flush is effectively single-worker (`inFlight > 0` gate), which constrains throughput and increases adaptation lag.

Fix:
- Introduce configurable multi-worker flush (`maxWorkers`), each pulling bounded batches.
- Keep retries with capped attempts and jittered backoff.

### 8.2 Two-speed ingestion strategy

Update:
- `src/services/tier2-service.ts`
- `src/services/qdrant-service.ts`
- `src/services/tier2-queue-service.ts`

Approach:
- Fast path (always): ingest payload + sparse/text representation quickly.
- Selective dense path: compute dense embeddings immediately only for:
  - high severity/confidence anomalies,
  - unseen or low-frequency `rhythm_hash`,
  - canary sample of normal events.
- Backfill path: async dense embedding for deferred events via lower-priority queue.

Result:
- Lower p95/p99 ingestion latency without dropping high-value semantic coverage.

### 8.3 Embedding cache and dedupe

Update:
- `src/services/qdrant-service.ts`

Add:
- Short-lived cache keyed by normalized text hash:
  - key: `xxhash64(normalized_text)`
  - value: embedding vector
  - TTL: 5-30 minutes (configurable)
- In-batch dedupe: compute unique texts before embedding call and remap after response.

Result:
- Large reduction in duplicate embedding work for repeated incidents/templates.

### 8.4 Candidate-first retrieval (dense rerank, not dense-first)

Update:
- `src/services/qdrant-service.ts`
- `src/services/forensic-analysis-service.ts`

Change query strategy:
1. Stage A: filter + sparse/full-text candidate retrieval (cheap, broad).
2. Stage B: dense rerank on top-K candidates (precise, bounded cost).

Also:
- Use Qdrant server-side grouping API (`search_groups`) for `rhythm_hash` clustering where available, rather than grouping client-side.

Result:
- Faster incident atlas/radar queries while keeping triage precision.

### 8.5 Sparse representation quality upgrade

Update:
- `src/services/qdrant-service.ts`

Current issue:
- Sparse vector hash space is too small (`% 1000`), causing collisions and recall loss.

Fix options:
- Increase sparse hash space significantly (for example 16k/32k).
- Prefer Qdrant text index + BM25 and use sparse vector as optional signal.
- Keep tokenizer normalization stable to preserve explainability and cache hit rate.

### 8.6 Queue prioritization and loss policy

Update:
- `src/services/tier2-queue-service.ts`
- `src/config/settings.ts`

Add queue classes:
- `critical`: high-severity/analyzed-now.
- `normal`: default anomaly flow.
- `backfill`: deferred embedding/backfill tasks.

Policy:
- Never drop `critical` first.
- Shed/defer `backfill` first during pressure.
- Track age and queue lag metrics by class.

### 8.7 Data model additions for embedding lifecycle

Update:
- `src/db/schema.ts`
- `src/db/registry.ts`
- `src/types.ts`

Add metadata fields:
- `embedding_status`: `pending | complete | failed | deferred`
- `embedding_model`
- `embedding_ts`
- `embedding_attempts`
- `tier` (`critical|normal|backfill`)

Purpose:
- Make partial ingestion explicit and auditable.

### 8.8 Config knobs to expose

Update:
- `src/config/settings.ts`

Add:
- `embedding.batchSize`
- `embedding.maxConcurrency`
- `embedding.cacheTtlSec`
- `embedding.maxRetries`
- `queue.maxWorkers`
- `queue.priorityWeights`
- `qdrant.maxConcurrentUpserts`

### 8.9 Observability and SLOs

Update:
- `src/services/evaluation-service.ts`
- `src/api/routes/analysis.ts`
- `via-core/crates/via-core/src/gatekeeper.rs` (policy/feedback side metrics)

Track:
- Embedding latency p50/p95/p99
- Embedding cache hit rate
- Queue lag by class
- Deferred/backfill completion time
- Dense rerank candidate count distribution
- Incident decision latency
- Precision/recall proxy before/after deferred embedding

### 8.10 Execution order for this section

1. Add embedding batching + limiter + cache in `qdrant-service.ts`.  
2. Add multi-worker prioritized queue in `tier2-queue-service.ts`.  
3. Switch retrieval to candidate-first + dense rerank.  
4. Add embedding lifecycle fields in DB and propagate through services.  
5. Add SLO dashboards and canary policy checks tied to these metrics.
