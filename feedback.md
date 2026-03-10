Yes, you nailed the "Holy Trinity" of observability. If you want any new microservice to drop seamlessly into this beast of an infrastructure you've built, those three are non-negotiable.

But because you have already built a custom Machine Context Protocol (MCP) fabric and a centralized routing agent (Alloy), there are a few extra rules you need to enforce for any new service you spin up.

Here is the exact checklist for every new microservice you add to this OS:

### 1. The Observability Non-Negotiables

* **W3C Trace Context Propagation:** This is the most critical part. When your API Gateway calls Service A, and Service A calls Service B, they must pass the `traceparent` HTTP header. If you drop this header, your Tempo waterfall graph breaks, and you won't be able to track cross-service latency.
* **Trace-Enriched Structured Logs:** Spit everything out to `stdout` as JSON. Every single log line must include the `traceId` and `spanId`. Do not write to local log files; let Alloy capture `stdout` and route it to Loki and your VIA gatekeeper.
* **Standardized RED Metrics:** Expose a `/metrics` endpoint for Prometheus/Alloy to scrape. Track Rate (requests per second), Errors (4xx/5xx status codes), and Duration (histograms of response times).

### 2. Zero-Trust Auth via Your MCP Fabric

Do not invent a new way to authenticate internal services. You already built a high-performance M2M identity fabric.

* **Use the SDKs:** Whether you are writing the new service in Python or TypeScript, use the MCP SDKs you built.
* **Stateless Validation:** Configure the new service as an `MCPResourceServer`. When internal services talk to it, they should send Ed25519-signed JWTs. This allows the receiving service to validate the token locally in ~0.1ms using the Web Crypto API or local JWKS verification, completely avoiding a 35ms network hop back to the Mono-Auth server.

### 3. Infrastructure Hygiene

* **Strict Statelessness:** Keep the new services 100% stateless so they can survive on your 1-core/1GB Oracle nodes. If they need state, point them to the central Postgres/Redis box.
* **Tailscale Binding:** Hardcode the services to only listen on your `100.x.x.x` tailnet IPs. They should be physically unreachable from the public internet.

If you strictly follow this pattern, any new service—whether it's a Python-based RAG agent or a Rust-based chunker—will instantly appear in your Grafana dashboards, inherit your zero-trust security model, and feed telemetry directly into your VIA anomaly detector without a single line of custom infrastructure code.

Since you've got the Python and TS SDKs ready to go, would you like to draft a boilerplate template (e.g., a minimal Bun or FastAPI setup) that already has the OTel trace propagation, logging, and MCP stateless JWT validation wired up?

----------------------------------------------------------------------------
================================================================================
# VIA Anomaly Detection: Architecture Findings & Path Forward
================================================================================

## Executive Summary

After analyzing the VIA-typescript anomaly detection system alongside mono-auth's
existing LGTM (Loki, Grafana, Tempo, Prometheus) observability stack, the 
following findings establish a clear architectural path forward.

---

## 1. What VIA Adds (That LGTM Cannot Do)

| Capability | LGTM (Prometheus/Loki/Tempo) | VIA Core (Rust Engine) |
|-------------|------------------------------|------------------------|
| **Per-entity baseline** | ❌ Global thresholds only | ✅ Learns each entity's normal behavior |
| **Statistical anomaly** | ❌ Fixed rules | ✅ 3σ deviation from entity's history |
| **Multi-variate** | ❌ Single metric rules | ✅ RRCF finds anomalous combinations |
| **Novel detection** | ❌ Must write rules | ✅ Unsupervised - learns automatically |
| **Latency** | N/A | ✅ Sub-30μs per event |

**Example:**
- User A normally makes 10 req/min. Suddenly makes 300 req/min.
- **Prometheus:** Won't alert - aggregate looks normal
- **VIA:** Learns User A's baseline → detects anomaly immediately

---

## 2. OTel as the Industry Standard

Yes, OTel should be the first priority. It covers:

| Telemetry Type | OTel Maturity | Notes |
|---------------|---------------|-------|
| Traces | ~99% mature | Industry standard |
| Metrics | ~95% mature | Prometheus compatible |
| Logs | ~80% mature | OTel + Loki JSON common |
| Profiles | ~30% new | Emerging |

**Conclusion:** Your current setup (Alloy → LGTM via OTLP) is correct. VIA Tier1 
should also accept OTel format (`/ingest/otel` endpoint already exists).

---

## 3. Proposed Architecture

### The Detection Stack

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SRE Agent Input                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│   "I see anomaly #1234 - what do I do?"                                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TIER 2: The Brain                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │  Incident   │  │  Correlation │  │   Context   │  │   Action    │       │
│   │  Builder    │  │   Engine     │  │   Enricher  │  │  Suggestor  │       │
│   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘       │
│          │                 │                 │                 │            │
│          ▼                 ▼                 ▼                 ▼            │
│   Groups related      Links traces    Adds: logs,      Suggests:         │
│   anomalies into      to anomalies    metrics,         - rollback        │
│   incidents           via traceId      config,          - scale           │
│                                         history         - restart         │
│                                                         - investigate      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▲
┌─────────────────────────────────────────────────────────────────────────────┐
│                      TIER 1: The Trigger                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│   Per-node Rust engine                                                     │
│   - Sub-30μs detection                                                      │
│   - 10 statistical detectors                                               │
│   - Outputs: AnomalySignal { entity, score, detectors_fired, context }    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Deployment Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    K8s Cluster (Cloud / Edge)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Node 1                                                             │   │
│  │  ┌──────────┐    ┌──────────┐                                      │   │
│  │  │  App     │───▶│  Alloy   │                                      │   │
│  │  │ (mono-   │    │          │──▶ OTLP ──▶ Homelab                 │   │
│  │  │  auth)   │    │          │      (Loki/Tempo/Prometheus)         │   │
│  │  └──────────┘    │          │                                      │   │
│  │                   │          │──▶ Anomaly───▶ Homelab              │   │
│  │                   │  Tier1   │      Signals   (Tier2 + Grafana)   │   │
│  │                   │  (Rust)  │                                      │   │
│  │                   └──────────┘                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Repeat per node → signals aggregate to central Tier2                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Minimal Tier 2 Restructure (For SRE Agent)

| Current Tier2 Feature | Keep? | Reason |
|----------------------|-------|--------|
| Qdrant clustering | ❌ | Semantic search not needed for SRE |
| Feedback loop | ✅ | Tune Tier1 detection |
| Policy compiler | ✅ | Action rules |
| Incident service | ✅ | Deduplication |
| Forensic analysis | ✅ | Root cause context |
| **NEW: Log/Trace enricher** | ✅ | Connect to LGTM |

### Tier 2 API Contract (For SRE Agent)

```
SRE Agent Question          │  Tier 2 Responsibility
────────────────────────────┼─────────────────────────────────
"What broke?"               │  Link anomaly → trace → service
"How bad?"                  │  Severity + blast radius (count)
"Why?"                      │  Pull related logs + metrics
"What worked before?"      │  Historical pattern matching
"What should I do?"         │  Action playbook lookup
"Did it work?"             │  Feedback loop to Tier1
```

---

## 5. What Complements LGTM

| LGTM Strength | VIA Complement |
|---------------|-----------------|
| "CPU > 80% for 5min" | "User X deviated 3σ from their normal" |
| "Error rate > 5%" | "Service Y's latency pattern changed unexpectedly" |
| "Log contains ERROR" | "This log is anomalous compared to baseline" |
| "Span duration P99 > 2s" | "This trace's structure is abnormal" |

---

## 6. Summary: The Clear Path

1. **Tier1 (Rust)**: Deploy as DaemonSet per node, ~5MB binary, reads same OTLP 
   stream as Alloy, emits anomaly signals to Tier2

2. **Tier2**: Strip down to Incident Builder + Context Enricher that queries 
   existing LGTM APIs (Loki/Tempo/Prometheus)

3. **Integration**: Alloy fans out to both storage AND local Tier1

4. **Output**: Structured JSON that SRE agent can consume

**You're not duplicating LGTM. You're adding statistical anomaly detection that 
rules-based alerting cannot express.**

---

## 7. Key Files Reference

- VIA Tier1: `D:\Sativa\VIA-typescript\via-core\crates\via-core\src\gatekeeper.rs`
- VIA Tier2: `D:\Sativa\VIA-typescript\src\services\tier2-service.ts`
- Current Alloy: `D:\Sativa\mono-auth\observability\alloy\alloy.config`
- Observability: `D:\Sativa\mono-auth\observability\README.md`

---
End of findings.