# VIA Frontend: "Cyber-SRE" Command Center Plan

This document outlines the architectural and design strategy for the VIA Frontend, focusing on deep integration with the existing LGTM (Loki, Grafana, Tempo, Prometheus) observability stack.

## 1. Technical Architecture

*   **Framework:** React 19 (using `use`, Actions, and Server Components where applicable).
*   **Build Tool:** Vite (fast HMR, optimized production builds).
*   **Styling:** Modular Vanilla CSS (Cyber-SRE dark theme: high-contrast, data-dense, neon accents).
*   **API Layer:** Hono RPC (100% type-safe shared types with the Bun backend).
*   **State Management:** TanStack Query (React Query) for server state and real-time updates.

## 2. Visual & UX Strategy

*   **Aesthetic:** "Glassmorphism" on dark backgrounds, monospace fonts for data, and high-frequency sparklines.
*   **Anomaly Radar:** A visual cluster view (Qdrant semantic groups) to see related issues at a glance.
*   **Live Stream:** A vertical scrolling feed of raw signals from Tier-1 for real-time monitoring.

## 3. The "Holy Trinity" Integration (LGTM)

Deep links and embedded context from `D:\Sativa\mono-auth\observability`:

*   **Tempo (Traces):** Every anomaly/incident card will have a "Trace Deep Link" that opens the specific `traceId` in your existing Grafana Tempo dashboard.
*   **Loki (Logs):** An embedded "Forensic Log" component that queries Loki for structured logs matching the `traceId` and timestamp window of the anomaly.
*   **Prometheus (Metrics):** Automatic generation of "RED" (Rate, Error, Duration) metrics around the incident window, showing the impact on the specific service.

## 4. AgentHub Task Breakdown (Execution Path)

To avoid file overlap and allow parallel development by AgentHub agents:

### [Task A] Foundation & Shell
*   **Goal:** Scaffold the Vite app, setup Hono RPC client, and define the global CSS theme.
*   **Scope:** `/frontend/src/main.tsx`, `/frontend/src/api/rpc.ts`, `/frontend/src/styles/theme.css`.

### [Task B] Incident & Anomaly Browser
*   **Goal:** The primary data grid/list for browsing detected issues.
*   **Scope:** `/frontend/src/features/incidents/*`.
*   **Integration:** Connects to `GET /analysis/incidents`.

### [Task C] Forensic Detail View (LGTM Bridge)
*   **Goal:** The deep-dive page for a single incident.
*   **Scope:** `/frontend/src/features/forensics/*`.
*   **Integration:** Implements the link generation for Tempo/Loki/Grafana.

### [Task D] Policy Studio (Tier-1 Control)
*   **Goal:** UI to tune Rust detector thresholds and weights.
*   **Scope:** `/frontend/src/features/policy/*`.
*   **Integration:** Connects to `POST /control/policy/publish`.

### [Task E] High-Performance Signal Stream
*   **Goal:** Real-time ticker of Tier-1 signals.
*   **Scope:** `/frontend/src/features/stream/*`.
*   **Integration:** Uses SSE or high-frequency polling from `src/api/routes/stream.ts`.

## 5. Deployment
*   The frontend will be served by the Bun backend in production (static assets) or as a standalone Vite dev server during development.
