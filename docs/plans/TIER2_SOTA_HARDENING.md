# Tier-2 SOTA Hardening (Implemented)

## Scope
Tier-2 is now designed as an event-driven intelligence and incident-management pipeline, with Tier-1 as the sole simulation/detection owner.

## Implemented Phases

### Phase A: Canonical Contract + Queue
- Introduced canonical Tier-2 event model with stable `eventId`, tenant/entity semantics, and detector context.
- Added bounded in-memory queue with:
  - dedupe window,
  - max queue size backpressure,
  - retries (max 3 attempts),
  - dead-letter writes.
- API now enqueues and returns accepted/rejected state with event ID.

### Phase B: Correlation Engine v2
- Replaced naive pairwise correlation path with bounded candidate generation:
  - trace ID groups,
  - rhythm hash groups,
  - temporal buckets (5-minute windows).
- Emits incident candidates with confidence and evidence payload.

### Phase C: Incident State + Decisions
- Added incident lifecycle persistence:
  - `tier2_incidents`
  - `tier2_decisions`
  - `tier2_dead_letters`
- Added deterministic policy engine:
  - `new`
  - `merged`
  - `escalated`
- Added APIs for incident listing/detail and pipeline observability.

### Phase D: Unified Benchmark Hooks
- Tier-2 simulation code and endpoints are removed.
- Simulation ownership is in Tier-1 for end-to-end benchmarking.

### Phase E: Reliability Hardening
- Startup now ensures required registry tables exist.
- Qdrant indexing expanded for incident workflow fields:
  - `event_id`
  - `entity_id`
- Graceful startup/shutdown includes queue lifecycle.

## Operational Endpoints
- Ingest: `POST /tier2/anomalies`
- Incidents: `GET /analysis/incidents`
- Incident detail: `GET /analysis/incidents/:incidentId`
- Queue stats: `GET /analysis/pipeline/stats`
- Dead letters: `GET /analysis/pipeline/dead-letters`

## Remaining Work (for production scale)
- Move queue from memory to durable broker (NATS/Kafka/Redis streams).
- Add per-tenant SLO limits and quota enforcement.
- Add replay tooling from dead-letter payloads.
- Add end-to-end benchmark scorer integrating Tier-1 truth labels and Tier-2 incident outcomes.
