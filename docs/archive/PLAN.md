# Detailed Implementation Plan for VIA v2

This document provides a detailed, step-by-step plan for upgrading the VeriStamp Incident Atlas (VIA) to its next-generation architecture. The plan is divided into four main phases, designed to be executed sequentially to ensure a stable and incremental rollout.

---

## Phase 1: Fortifying the Foundation (Ingestion & Tier-1)

**Goal:** Replace the core detection engine with a real-time, in-memory system and make the ingestion path more robust.

1.  **Implement Ingestion Queue:**
    *   In `app/main.py`, create a global `asyncio.Queue`.
    *   In `app/api/v1/endpoints/ingest.py`, modify the `ingest_stream` endpoint. Instead of calling the `IngestionService` directly, it will now simply `await queue.put(log_batch)` and return `200 OK`.
    *   In `app/worker.py`, create a new background task (`process_ingestion_queue`) that runs in a loop, pulls batches from the queue (`await queue.get()`), and passes them to the `IngestionService`.

2.  **Create the New `Tier1Engine`:**
    *   Create a new file: `app/services/tier1_engine.py`.
    *   Define the core classes:
        *   `AnomalyProfile`: A Pydantic model to hold the live statistical models for a single `rhythm_hash` (EWMA for frequency, HyperLogLog for cardinality, CUSUM for drift).
        *   `StateManager`: A class to manage the in-memory dictionary of all active `AnomalyProfile` objects.
        *   `LiveDetectionEngine`: The class containing the logic to run checks against a profile.
        *   `Tier1Engine`: The main class that orchestrates the above components.

3.  **Upgrade the `IngestionService`:**
    *   In `app/services/ingestion_service.py`, gut the existing logic that writes to Qdrant.
    *   Its new role is to parse the incoming log batch, perform Rhythm Hashing, and create a lightweight `BatchSummary` object as described in the PRD.
    *   At the end of the method, it will call `tier1_engine.process_summary(summary)`.

4.  **Deprecate the `RhythmAnalysisService`:**
    *   The `RhythmAnalysisService` and its periodic background task in `app/worker.py` should be completely removed. Its functionality is now live within the `Tier1Engine`.

5.  **Update the `PromotionService`:**
    *   Modify the `promote_anomalies` method to accept the new `AnomalySignal` object from the `Tier1Engine` instead of the old format.

---

## Phase 2: Upgrading the Schema Service & UI

**Goal:** Enhance the data onboarding process to include automatic behavioral profiling.

1.  **Enhance the `SchemaService` Backend:**
    *   In `app/services/schema_service.py`, modify the `detect_schema` method to perform the new two-stage process.
    *   **Stage 1 (Structural):** Retain the existing logic to parse the file and determine the structure.
    *   **Stage 2 (Behavioral):** After parsing, process the full list of log objects. Calculate frequency (events/sec) and cardinality for key variables for the most significant `rhythm_hash`es.
    *   The method should now return both the structural schema and the proposed `UnifiedServiceAnomalyProfile` (USAP) as a JSON object.

2.  **Upgrade the "Data Sources" UI:**
    *   In `ui.py`, update the "Data Sources" tab.
    *   Add a new UI element (e.g., a `gr.JSON` component or a custom-built accordion with `gr.Textbox` for editing) to display the proposed behavioral profile returned from the backend.
    *   Modify the "Save" button's logic to save both the structural schema and the new behavioral profile to the `registry.db`.

---

## Phase 3: Building the Simulation & Evaluation Framework

**Goal:** Create a powerful, user-controllable simulation engine and a dashboard to validate its effectiveness.

1.  **Architect the Simulation Control Plane:**
    *   In `otel_mock/main.py`, refactor the application to be scenario-based.
    *   Create a `scenarios` directory within `otel_mock`.
    *   Define a base `Scenario` class with `start`, `stop`, and `get_next_log_batch` methods.
    *   Implement the detailed scenarios from the PRD (Credential Stuffing, Cascading Failure, etc.) as subclasses of `Scenario`.
    *   Create new FastAPI endpoints: `GET /scenarios`, `POST /scenarios/{name}/start`, and `POST /scenarios/stop` to manage the simulation.

2.  **Implement the Evaluation Service:**
    *   This requires a parallel "ground truth" stream from the simulator. When a scenario injects an anomaly, it should also write a record to a separate, simple store (e.g., a local SQLite DB or even a CSV file) that VIA cannot see.
    *   In `app/worker.py`, create a new background task (`run_evaluation`) that runs every minute.
    *   This task will: 
        1. Fetch the latest anomalies detected by VIA from Tier-2.
        2. Fetch the ground truth labels from the simulator's private store for the same time window.
        3. Calculate Precision, Recall, and F1-Score.
        4. Store these metrics in a new table in the `registry.db`.

3.  **Build the Evaluation UI:**
    *   In `ui.py`, create a new tab: "System Evaluation".
    *   Add a "Simulation Control" section with a dropdown to list scenarios from `GET /scenarios` and buttons to call the `start`/`stop` endpoints.
    *   Add a "Live Performance" section with `gr.Number` or `gr.Gauge` components to display the latest Precision, Recall, and F1-Score by querying the results from the Evaluation Service.

---

## Phase 4: Evolving Tier-2 to an Incident Graph

**Goal:** Transform the Tier-2 archive into an automated detective that reveals the full story behind incidents.

1.  **Create the Correlation Engine:**
    *   In `app/worker.py`, add a new periodic task (`run_incident_correlation`).
    *   This task will call a new method in `app/services/forensic_analysis_service.py`.
    *   The method will query for new, un-analyzed anomalies from Qdrant.
    *   It will use temporal, trace ID, and semantic linking (via `QdrantService.recommend`) to find connections between them.

2.  **Implement the `IncidentGraphStore`:**
    *   In `app/db/registry.py`, define a new SQLAlchemy model for the `incident_graph` table. This table will store the relationships between individual Qdrant point IDs and the "meta-incident" IDs they belong to.
    *   When the Correlation Engine finds links, it will write them to this table.

3.  **Develop the Incident Graph UI:**
    *   In `ui.py`, enhance the "Atlas" view.
    *   When displaying incident clusters, the UI will make a new backend call to check if a cluster is part of a larger meta-incident.
    *   If it is, display a prominent icon or banner.
    *   Create a new modal or view that is triggered on click. This view will fetch the full graph data and use a library (e.g., `pyvis` or a custom Gradio component) to visualize the connections and timeline of the full incident, telling the complete story to the operator.
