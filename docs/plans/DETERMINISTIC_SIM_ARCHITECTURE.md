# Deterministic Simulation Architecture (Tier1/Tier2 E2E)

## Objective

Build a simulation and benchmarking system where the same input plan produces the same simulated stream and the same evaluation outputs, so precision/recall/F1 can be trusted for architecture decisions.

## Why Current Design Is Insufficient

What is true today:

1. Scenario randomness uses `rand::rng()` in multiple scenario files, which is non-deterministic per run.
2. Scenario creation is name-driven with default parameters (`create_scenario(name)`), but there is no run manifest with exact seed + parameters.
3. Bench output is strong at aggregate level, but reproducibility guarantees are implicit rather than enforced.

Result:

- Schedules are controlled, but random streams are not reproducible by contract.
- Two runs with same scenario names can diverge in event-level behavior.

## Industry-Grade Design Principles

1. Reproducibility is a first-class API contract, not a side effect.
2. A simulation run must be fully describable by a portable manifest.
3. Every random choice must come from a seeded deterministic RNG hierarchy.
4. Event identity must be deterministic and stable.
5. Metrics must include both aggregate and per-injected-anomaly accountability.
6. Non-deterministic mode can exist, but deterministic mode must be default for benchmarks.

## Target Architecture

### 1) Simulation Run Manifest (Single Source of Truth)

Define a serializable `SimulationRunManifest` consumed by `via-sim` and `via-bench`:

- `run_id`: string
- `sim_version`: string (schema/version pin)
- `global_seed`: `u64`
- `base_scenario`: typed config (name + parameters)
- `anomalies`: ordered list with
  - `anomaly_id`
  - `scenario config` (typed parameters)
  - `start_offset_ns`
  - `duration_ns`
- `tick_ns`
- `duration_ns`
- `clock_mode`: `fixed` (benchmark) or `wall` (interactive)

This manifest is saved with benchmark results.

### 2) Deterministic RNG Hierarchy

Use seeded RNG streams derived from `global_seed`:

- Engine RNG stream (for orchestration only)
- Scenario RNG stream per scenario instance
- Optional sub-stream per concern (timing/message/attribute) if needed

Rule:

- Scenarios never call `rand::rng()`.
- Scenarios receive and retain their own seeded RNG in struct state.

Recommended from `rand` docs (Context7 `/rust-random/rand`):

- use `SeedableRng` and fixed seeds for reproducibility.

### 3) Typed Scenario Configs

Replace string-only scenario constructors with typed config structs:

- `NormalTrafficConfig`
- `CredentialStuffingConfig`
- etc.

Each scenario instance stores:

- immutable config
- seeded RNG state
- internal deterministic state machine counters

### 4) Deterministic Event IDs

For every generated log/event, compute deterministic IDs from:

- `run_id`
- `scenario_instance_id`
- logical sequence number
- simulation timestamp

Do not use UUID v4 for benchmark mode.

### 5) Engine Modes

Provide explicit modes:

1. `BenchmarkDeterministic` (default for `via-bench`)
2. `InteractiveNonDeterministic` (optional for demos)

In deterministic mode:

- start time is logical (`0`) plus tick progression.
- all IDs and randomness are deterministic.

### 6) Benchmark Result Contract

Store:

1. Manifest snapshot
2. Engine build/version metadata
3. Aggregate metrics
4. Per-anomaly metrics
5. Optional sample mismatches

Per-anomaly metrics minimum:

- `ground_truth_events`
- `detected_events`
- `missed_events`
- `recall`

## Boundaries: What Stays in Rust vs Elsewhere

### Primary Benchmark Mode (Recommended)

Run the production services as black boxes:

1. external deterministic simulator emits logs/events
2. send into Tier-1 via public ingest API
3. allow normal Tier-1 -> Tier-2 forwarding path
4. collect benchmark metrics from public analysis APIs and benchmark-tagged records

Rules:

- no in-process shortcuts in this mode
- no policy publish/rollback side effects
- all records tagged by benchmark `run_id`

### Keep in Rust (Implementation Location)

1. Event generation and anomaly injection (`via-sim`)
2. Tier1 scoring path benchmark (`via-bench` + `via-core`)
3. Deterministic benchmark orchestration

Reason:

- Same language/runtime as Tier1 avoids transport/runtime skew and preserves hot-path realism.

### Keep in Tier2/TS

1. Incident clustering and triage behavior
2. End-to-end pipeline ingestion performance under queue pressure
3. Policy feedback loops and control-plane behavior

Reason:

- This is where real Tier2 behavior exists; E2E validity requires this layer.

## Phased Implementation Plan

## Phase 1 (Hard Determinism Foundation)

1. Add manifest types in `via-sim`.
2. Add deterministic RNG plumbing to engine and all scenarios.
3. Remove direct `rand::rng()` usage in scenarios.
4. Add deterministic event ID generation path.
5. Add replay test: same manifest => byte-equal event stream digest.

Exit criterion:

- Re-running same manifest yields identical event hash and identical detection confusion matrix.

## Phase 2 (Config and Contracts)

1. Replace string scenario creation with typed configs.
2. Keep compatibility adapter for old names.
3. Persist manifest with benchmark outputs.

Exit criterion:

- Benchmark artifacts are self-contained and replayable.

## Phase 3 (E2E Trustability)

1. Enforce deterministic mode in `pipeline` benchmark command.
2. Add black-box pipeline mode as default; keep in-process mode for micro-benchmark only.
3. Add regression tests for known manifests with expected metrics bands.
4. Add CI check to detect deterministic drift.

Exit criterion:

- Any simulation drift becomes an explicit code-change signal.

## Testing Strategy

1. Unit: scenario seed reproducibility.
2. Property: deterministic mode is invariant to repeated executions.
3. Integration: full pipeline replay comparison by manifest.
4. Regression: golden digest snapshots for representative scenarios.

## Non-Goals

1. Perfect real-world randomness in deterministic mode.
2. Full elimination of non-deterministic interactive demos.

## Risks and Mitigations

1. Risk: refactor touches every scenario.
   - Mitigation: introduce scenario adapter layer and migrate incrementally.
2. Risk: existing benchmark baselines shift.
   - Mitigation: regenerate baselines after deterministic migration and freeze new manifests.
3. Risk: confusion between deterministic and demo mode.
   - Mitigation: explicit CLI flags and clear output header indicating mode.

## Immediate Next Changes Recommended

1. Add `DeterminismConfig` and `SimulationRunManifest` types.
2. Introduce seeded RNG into `SimulationEngine`.
3. Migrate one scenario family (`security`) first and add deterministic tests.
4. Then migrate remaining scenario families.
