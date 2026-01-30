# VIA v2 Bun Implementation Plan

> **Status**: Draft  
> **Target Runtime**: Bun 1.3.7+ (for JSON5/JSONL) or Bun 1.2+ (with fallbacks)  
> **Architecture**: Single-binary, type-safe, real-time anomaly detection  

---  

## Executive Summary

This document outlines the complete implementation plan for rewriting the VeriStamp Incident Atlas (VIA) using **Bun** instead of Python. The rewrite leverages Bun's native capabilities including JSON5 configuration parsing, JSONL streaming for log ingestion, and superior in-memory performance for the Tier-1 detection engine.

### Key Technology Choices

| Component | Python (Current) | Bun (Target) | Rationale |
|-----------|-----------------|--------------|-----------|
| **Runtime** | Python 3.12 | Bun 1.3.7+ | Single binary, faster startup, lower memory |
| **HTTP Framework** | FastAPI | Hono or Elysia | Bun-native, faster, simpler |
| **Config Format** | .env + Python | JSON5 (Bun 1.3.7+) or JSON | Native comments, trailing commas |
| **Log Ingestion**** | JSON parsing | `Bun.JSONL` (1.3.7+) or `jsonl` npm | Streaming, native performance |
| **Hashing** | Simhash | `Bun.hash.xxHash64` | 10x faster, native |
| **Embeddings** | fastembed | LM Studio API | Decoupled, language-agnostic |
| **Vector DB** | Qdrant Python client | Qdrant JS/REST | Same capabilities |
| **Registry DB** | SQLite | PostgreSQL | Production-grade, Drizzle ORM |
| **Frontend** | Gradio | TanStack SPA | Modern, decoupled |

---

## Bun Version Compatibility Matrix

| Feature | Bun 1.2 | Bun 1.3.7+ | Recommendation |
|---------|---------|------------|----------------|
| `Bun.hash.xxHash64()` | ✅ Native | ✅ Native | Use native |
| `Bun.serve()` | ✅ Native | ✅ Native | Use native |
| `Bun.JSON5.parse()` | ❌ | ✅ Native | Use `json5` npm for 1.2 |
| `Bun.JSONL.parse()` | ❌ | ✅ Native | Use `jsonl` npm for 1.2 |
| `Bun.JSONL.parseChunk()` | ❌ | ✅ Native | Manual streaming for 1.2 |
| Single binary compile | ✅ | ✅ | Both support |
| TypeScript support | ✅ | ✅ | Both support |

### Quick Decision Guide

**Use Bun 1.3.7+ if:**
- ✅ You want native JSON5/JSONL support (no npm deps)
- ✅ You want the latest performance improvements
- ✅ You're starting fresh (no existing Bun 1.2 deployment)

**Use Bun 1.2 if:**
- ✅ You have existing Bun 1.2 infrastructure
- ✅ You need to stay on a specific version for compatibility
- ✅ You're okay with adding `json5` and `jsonl` npm packages

**Both versions work perfectly for:**
- ✅ Core statistical algorithms (EWMA, HLL, CUSUM)
- ✅ Fast hashing with `Bun.hash.xxHash64()`
- ✅ HTTP server with `Bun.serve()`
- ✅ Single binary compilation
- ✅ PostgreSQL + Drizzle ORM
- ✅ Qdrant REST API integration

---

## Bun Native Features We'll Leverage

### 1. JSON5 Configuration

**Bun 1.3.7+ (Native):**
```typescript
// config/settings.json5
{
  // Tier-1 Detection Engine Settings
  tier1: {
    // EWMA half-life in seconds (controls smoothing)
    ewmaHalfLife: 60,
    
    // HyperLogLog precision (10-16, higher = more memory)
    hllPrecision: 14,  // ~12KB per rhythm_hash
    
    // CUSUM parameters for drift detection
    cusum: {
      slack: 2.0,       // Allowable deviation
      threshold: 5.0,   // Alarm threshold
    },
  },
  
  // Ingestion queue settings
  queue: {
    maxSize: 10000,     // Backpressure limit
    batchSize: 100,     // Process N logs at once
    flushInterval: 1000, // ms
  },
}
```

**Usage (Bun 1.3.7+):**
```typescript
import settings from './config/settings.json5';
// settings.tier1.ewmaHalfLife === 60
```

**Fallback for Bun 1.2:**
```bash
bun add json5
```

```typescript
// config/settings.ts
import JSON5 from 'json5';
import { readFileSync } from 'fs';

const raw = readFileSync('./config/settings.json5', 'utf-8');
export const settings = JSON5.parse(raw);
```

### 2. JSONL Streaming

**Bun 1.3.7+ (Native):**
```typescript
// Stream processing for real-time ingestion
async function* streamLogs(source: ReadableStream) {
  let buffer = '';
  const reader = source.getReader();
  
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    
    buffer += new TextDecoder().decode(value);
    const result = Bun.JSONL.parseChunk(buffer);
    
    for (const log of result.values) {
      yield log;  // Yield each parsed log record
    }
    
    // Keep unconsumed portion
    buffer = buffer.slice(result.read);
  }
}
```

**Fallback for Bun 1.2:**
```bash
bun add jsonl
```

```typescript
import { JSONL } from 'jsonl';

// Parse complete JSONL string
const logs = JSONL.parse(jsonlString);

// For streaming, use line-by-line parsing
async function* streamLogs(source: ReadableStream) {
  const reader = source.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    
    buffer += decoder.decode(value, { stream: true });
    
    // Process complete lines
    let newlineIndex;
    while ((newlineIndex = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, newlineIndex);
      buffer = buffer.slice(newlineIndex + 1);
      
      if (line.trim()) {
        yield JSON.parse(line);
      }
    }
  }
}
```

### 3. Native Hashing (`Bun.hash`)

**Available in Bun 1.2+:**
```typescript
function generateRhythmHash(
  service: string,
  severity: string,
  template: string
): string {
  const combined = `${service}:${severity}:${template}`;
  const hash = Bun.hash.xxHash64(combined);
  return hash.toString(16);  // Hex string
}
```

**Performance**: ~10x faster than Python's hashlib

---  

## Phase 1: Core Infrastructure & Tier-1 Engine

### 1.1 Project Structure

```
via-bun/
├── src/
│   ├── algorithms/           # Statistical models (custom)
│   │   ├── ewma.ts
│   │   ├── hyperloglog.ts
│   │   ├── cusum.ts
│   │   └── index.ts
│   ├── core/                 # Core domain models
│   │   ├── anomaly-profile.ts
│   │   ├── rhythm-hasher.ts
│   │   ├── state-manager.ts
│   │   └── batch-summary.ts
│   ├── services/             # Business logic
│   │   ├── tier1-engine.ts
│   │   ├── ingestion-service.ts
│   │   ├── embedding-client.ts
│   │   └── qdrant-service.ts
│   ├── db/                   # Database layer
│   │   ├── schema.ts         # Drizzle ORM
│   │   ├── registry.ts       # PostgreSQL client
│   │   └── migrations/
│   ├── api/                  # HTTP layer
│   │   ├── server.ts         # Hono/Elysia setup
│   │   ├── middleware/
│   │   └── routes/
│   │       ├── ingest.ts
│   │       ├── health.ts
│   │       └── control.ts
│   ├── queue/                # Async ingestion queue
│   │   ├── queue.ts
│   │   └── worker.ts
│   ├── config/               # JSON5 configurations
│   │   ├── settings.json5
│   │   ├── thresholds.json5
│   │   └── services.json5
│   └── utils/
│       └── logger.ts
├── tests/
├── drizzle.config.ts
├── bunfig.toml
└── package.json
```

### 1.2 Custom Statistical Algorithms

#### EWMA (Exponentially Weighted Moving Average)

```typescript
// src/algorithms/ewma.ts
export class EWMA {
  private alpha: number;
  private value: number | null = null;
  private count: number = 0;
  
  constructor(options: { halfLife: number } | { alpha: number }) {
    if ('halfLife' in options) {
      this.alpha = 1 - Math.exp(-Math.LN2 / options.halfLife);
    } else {
      this.alpha = options.alpha;
    }
  }
  
  update(sample: number): number {
    this.count++;
    if (this.value === null) {
      this.value = sample;
    } else {
      this.value = this.alpha * sample + (1 - this.alpha) * this.value;
    }
    return this.value;
  }
  
  getValue(): number | null {
    return this.value;
  }
  
  getVariance(): number {
    // Simplified variance estimate
    return this.value ? Math.abs(this.value) * 0.1 : 0;
  }
  
  reset(): void {
    this.value = null;
    this.count = 0;
  }
}
```

#### HyperLogLog (Cardinality Estimation)

```typescript
// src/algorithms/hyperloglog.ts
export class HyperLogLog {
  private registers: Uint8Array;
  private p: number;
  private m: number;
  private alphaMM: number;
  
  constructor(precision: number = 14) {
    this.p = Math.max(4, Math.min(16, precision));
    this.m = 1 << this.p;
    this.registers = new Uint8Array(this.m);
    
    // Alpha constant based on precision
    this.alphaMM = this.getAlpha() * this.m * this.m;
  }
  
  private getAlpha(): number {
    switch (this.p) {
      case 4: return 0.673;
      case 5: return 0.697;
      case 6: return 0.709;
      default: return 0.7213 / (1 + 1.079 / this.m);
    }
  }
  
  add(value: string): void {
    // Use Bun's native xxHash64
    const hash = Bun.hash.xxHash64(value);
    
    // Extract register index from first p bits
    const registerIndex = Number(hash >> BigInt(64 - this.p));
    
    // Count leading zeros in remaining bits
    const remaining = hash << BigInt(this.p);
    const leadingZeros = this.countLeadingZeros(remaining) + 1;
    
    // Update register with max value
    this.registers[registerIndex] = Math.max(
      this.registers[registerIndex],
      Math.min(leadingZeros, 64 - this.p + 1)
    );
  }
  
  private countLeadingZeros(value: bigint): number {
    if (value === 0n) return 64;
    let count = 0;
    let v = value;
    while ((v & 0x8000000000000000n) === 0n) {
      count++;
      v <<= 1n;
    }
    return count;
  }
  
  count(): number {
    const m = this.m;
    let rawSum = 0;
    let zeroRegisters = 0;
    
    for (let i = 0; i < m; i++) {
      rawSum += 1.0 / (1 << this.registers[i]);
      if (this.registers[i] === 0) zeroRegisters++;
    }
    
    const rawEstimate = this.alphaMM / rawSum;
    
    // Small range correction
    if (rawEstimate <= 2.5 * m && zeroRegisters !== 0) {
      return m * Math.log(m / zeroRegisters);
    }
    
    // Medium range - no correction
    if (rawEstimate <= (1 / 30) * (1 << 32)) {
      return rawEstimate;
    }
    
    // Large range correction
    return -(1 << 32) * Math.log(1 - rawEstimate / (1 << 32));
  }
  
  merge(other: HyperLogLog): void {
    for (let i = 0; i < this.m; i++) {
      this.registers[i] = Math.max(this.registers[i], other.registers[i]);
    }
  }
  
  serialize(): Uint8Array {
    return new Uint8Array(this.registers);
  }
  
  static deserialize(data: Uint8Array, precision: number): HyperLogLog {
    const hll = new HyperLogLog(precision);
    hll.registers.set(data);
    return hll;
  }
}
```

#### CUSUM (Cumulative Sum Control Chart)

```typescript
// src/algorithms/cusum.ts
export interface CUSUMOptions {
  target: number;      // Target mean (μ)
  slack: number;       // Allowable slack (K)
  threshold: number;   // Decision interval (H)
}

export class CUSUM {
  private target: number;
  private slack: number;
  private threshold: number;
  
  private cPos: number = 0;
  private cNeg: number = 0;
  
  public alarm: boolean = false;
  public alarmType: 'high' | 'low' | null = null;
  public alarmValue: number | null = null;
  
  constructor(options: CUSUMOptions) {
    this.target = options.target;
    this.slack = options.slack;
    this.threshold = options.threshold;
  }
  
  update(sample: number): boolean {
    // Reset alarm state
    this.alarm = false;
    this.alarmType = null;
    this.alarmValue = null;
    
    // Calculate deviation from target
    const deviation = sample - this.target;
    
    // Upper CUSUM: detects positive shifts (increases)
    this.cPos = Math.max(0, this.cPos + deviation - this.slack);
    
    // Lower CUSUM: detects negative shifts (decreases)
    this.cNeg = Math.max(0, this.cNeg - deviation - this.slack);
    
    // Check for alarms
    if (this.cPos > this.threshold) {
      this.alarm = true;
      this.alarmType = 'high';
      this.alarmValue = this.cPos;
      this.cPos = 0;  // Reset after alarm
      return true;
    }
    
    if (this.cNeg > this.threshold) {
      this.alarm = true;
      this.alarmType = 'low';
      this.alarmValue = this.cNeg;
      this.cNeg = 0;  // Reset after alarm
      return true;
    }
    
    return false;
  }
  
  getValues(): { upper: number; lower: number } {
    return { upper: this.cPos, lower: this.cNeg };
  }
  
  reset(): void {
    this.cPos = 0;
    this.cNeg = 0;
    this.alarm = false;
    this.alarmType = null;
    this.alarmValue = null;
  }
}
```

### 1.3 Core Domain Models

```typescript
// src/core/anomaly-profile.ts
import { EWMA } from '../algorithms/ewma';
import { HyperLogLog } from '../algorithms/hyperloglog';
import { CUSUM } from '../algorithms/cusum';
import type Settings from '../config/settings.json5';

export interface AnomalySignal {
  rhythmHash: string;
  service: string;
  severity: string;
  anomalyType: 'novelty' | 'frequency' | 'drift';
  confidence: number;
  context: string;
  timestamp: number;
  metadata: Record<string, unknown>;
}

export class AnomalyProfile {
  readonly rhythmHash: string;
  readonly service: string;
  
  // Statistical models
  private frequencyEWMA: EWMA;
  private cardinalityHLL: HyperLogLog;
  private driftCUSUM: CUSUM;
  
  // State
  private lastUpdated: number = 0;
  private eventCount: number = 0;
  private isBaselineEstablished: boolean = false;
  
  constructor(
    rhythmHash: string,
    service: string,
    config: Settings['tier1']
  ) {
    this.rhythmHash = rhythmHash;
    this.service = service;
    
    this.frequencyEWMA = new EWMA({ halfLife: config.ewmaHalfLife });
    this.cardinalityHLL = new HyperLogLog(config.hllPrecision);
    this.driftCUSUM = new CUSUM(config.cusum);
  }
  
  processEvent(timestamp: number, uniqueId: string): AnomalySignal | null {
    this.eventCount++;
    this.lastUpdated = timestamp;
    
    // Update cardinality estimator
    this.cardinalityHLL.add(uniqueId);
    
    // Update frequency model
    const currentFrequency = this.calculateCurrentFrequency();
    const smoothedFrequency = this.frequencyEWMA.update(currentFrequency);
    
    // Check for drift
    const driftDetected = this.driftCUSUM.update(currentFrequency);
    
    // Detect anomalies
    if (!this.isBaselineEstablished) {
      if (this.eventCount >= 10) {
        this.isBaselineEstablished = true;
      }
      return null;
    }
    
    // Check for frequency anomaly
    const expectedFreq = this.frequencyEWMA.getValue() || 0;
    const variance = this.frequencyEWMA.getVariance();
    const threshold = expectedFreq + 2.5 * Math.sqrt(variance || 1);
    
    if (currentFrequency > threshold && this.eventCount >= 3) {
      return {
        rhythmHash: this.rhythmHash,
        service: this.service,
        severity: 'WARN',
        anomalyType: 'frequency',
        confidence: Math.min(0.95, currentFrequency / threshold - 1),
        context: `Frequency ${currentFrequency.toFixed(1)} exceeds threshold ${threshold.toFixed(1)}`,
        timestamp,
        metadata: {
          expected: expectedFreq,
          actual: currentFrequency,
          variance,
        },
      };
    }
    
    // Check for drift anomaly
    if (driftDetected) {
      return {
        rhythmHash: this.rhythmHash,
        service: this.service,
        severity: 'WARN',
        anomalyType: 'drift',
        confidence: 0.85,
        context: `CUSUM drift detected: ${this.driftCUSUM.alarmType}`,
        timestamp,
        metadata: {
          cusumValues: this.driftCUSUM.getValues(),
        },
      };
    }
    
    return null;
  }
  
  private calculateCurrentFrequency(): number {
    // Calculate events per minute based on recent activity
    const now = Date.now() / 1000;
    const timeWindow = Math.max(1, now - this.lastUpdated);
    return this.eventCount / (timeWindow / 60);
  }
  
  getCardinality(): number {
    return this.cardinalityHLL.count();
  }
  
  getStats() {
    return {
      rhythmHash: this.rhythmHash,
      eventCount: this.eventCount,
      cardinality: this.getCardinality(),
      frequency: this.frequencyEWMA.getValue(),
      lastUpdated: this.lastUpdated,
      isBaselineEstablished: this.isBaselineEstablished,
    };
  }
}
```

### 1.4 In-Memory State Manager

```typescript
// src/core/state-manager.ts
import { AnomalyProfile } from './anomaly-profile';
import type settings from '../config/settings.json5';

export class StateManager {
  private profiles: Map<string, AnomalyProfile> = new Map();
  private config: typeof settings;
  
  constructor(config: typeof settings) {
    this.config = config;
  }
  
  getOrCreateProfile(rhythmHash: string, service: string): AnomalyProfile {
    let profile = this.profiles.get(rhythmHash);
    if (!profile) {
      profile = new AnomalyProfile(rhythmHash, service, this.config.tier1);
      this.profiles.set(rhythmHash, profile);
    }
    return profile;
  }
  
  getProfile(rhythmHash: string): AnomalyProfile | undefined {
    return this.profiles.get(rhythmHash);
  }
  
  getAllProfiles(): AnomalyProfile[] {
    return Array.from(this.profiles.values());
  }
  
  getStats() {
    return {
      totalProfiles: this.profiles.size,
      memoryEstimateMB: this.estimateMemoryUsage(),
    };
  }
  
  private estimateMemoryUsage(): number {
    // Rough estimate: ~15KB per profile (HLL registers + overhead)
    return (this.profiles.size * 15) / 1024;
  }
  
  // Cleanup old profiles (call periodically)
  cleanup(maxAgeSeconds: number): number {
    const now = Date.now() / 1000;
    let removed = 0;
    
    for (const [hash, profile] of this.profiles) {
      // Access private field via type assertion for cleanup
      const lastUpdated = (profile as unknown as { lastUpdated: number }).lastUpdated;
      if (now - lastUpdated > maxAgeSeconds) {
        this.profiles.delete(hash);
        removed++;
      }
    }
    
    return removed;
  }
}
```

---  

## Phase 2: Tier-1 Engine & Ingestion Queue

### 2.1 Async Queue Implementation

```typescript
// src/queue/queue.ts
export interface QueueItem {
  id: string;
  timestamp: number;
  data: unknown;
}

export class AsyncQueue<T> {
  private queue: T[] = [];
  private resolvers: ((value: T) => void)[] = [];
  private maxSize: number;
  
  constructor(maxSize: number = 10000) {
    this.maxSize = maxSize;
  }
  
  async put(item: T): Promise<void> {
    // If there are waiting consumers, give directly
    if (this.resolvers.length > 0) {
      const resolve = this.resolvers.shift()!;
      resolve(item);
      return;
    }
    
    // Check backpressure
    if (this.queue.length >= this.maxSize) {
      throw new Error('Queue full - backpressure detected');
    }
    
    this.queue.push(item);
  }
  
  async get(): Promise<T> {
    // If items available, return immediately
    if (this.queue.length > 0) {
      return this.queue.shift()!;
    }
    
    // Wait for new item
    return new Promise((resolve) => {
      this.resolvers.push(resolve);
    });
  }
  
  get size(): number {
    return this.queue.length;
  }
  
  get isEmpty(): boolean {
    return this.queue.length === 0;
  }
}
```

### 2.2 Queue Worker

```typescript
// src/queue/worker.ts
import { AsyncQueue } from './queue';
import { Tier1Engine } from '../services/tier1-engine';
import type { LogRecord } from '../services/ingestion-service';

export class QueueWorker {
  private queue: AsyncQueue<LogRecord>;
  private engine: Tier1Engine;
  private isRunning: boolean = false;
  private batchSize: number;
  private flushInterval: number;
  
  constructor(
    engine: Tier1Engine,
    options: { batchSize: number; flushInterval: number }
  ) {
    this.queue = new AsyncQueue(10000);
    this.engine = engine;
    this.batchSize = options.batchSize;
    this.flushInterval = options.flushInterval;
  }
  
  async enqueue(log: LogRecord): Promise<void> {
    await this.queue.put(log);
  }
  
  start(): void {
    if (this.isRunning) return;
    this.isRunning = true;
    
    // Start batch processor
    this.runBatchLoop();
    
    // Start periodic flush
    setInterval(() => this.flush(), this.flushInterval);
  }
  
  stop(): void {
    this.isRunning = false;
  }
  
  private async runBatchLoop(): Promise<void> {
    const batch: LogRecord[] = [];
    
    while (this.isRunning) {
      try {
        const log = await this.queue.get();
        batch.push(log);
        
        if (batch.length >= this.batchSize) {
          await this.processBatch(batch);
          batch.length = 0;
        }
      } catch (error) {
        console.error('Worker error:', error);
      }
    }
  }
  
  private async processBatch(batch: LogRecord[]): Promise<void> {
    const summary = this.createBatchSummary(batch);
    await this.engine.processSummary(summary);
  }
  
  private async flush(): Promise<void> {
    // Process any remaining items
    while (!this.queue.isEmpty) {
      const batch: LogRecord[] = [];
      while (batch.length < this.batchSize && !this.queue.isEmpty) {
        // Non-blocking get for flush
        const item = this.queue.get();
        batch.push(await item);
      }
      await this.processBatch(batch);
    }
  }
  
  private createBatchSummary(batch: LogRecord[]) {
    // Group by rhythm_hash
    const groups = new Map<string, LogRecord[]>();
    
    for (const log of batch) {
      const hash = log.rhythmHash;
      if (!groups.has(hash)) {
        groups.set(hash, []);
      }
      groups.get(hash)!.push(log);
    }
    
    return {
      timestamp: Date.now() / 1000,
      totalLogs: batch.length,
      groups: Array.from(groups.entries()).map(([hash, logs]) => ({
        rhythmHash: hash,
        service: logs[0].service,
        count: logs.length,
        uniqueIds: logs.map(l => l.id),
      })),
    };
  }
}
```

---  

## Phase 3: Database Layer (PostgreSQL + Drizzle)

### 3.1 Schema Definition

```typescript
// src/db/schema.ts
import { pgTable, serial, text, integer, boolean, jsonb, timestamp } from 'drizzle-orm/pg-core';

// Schema registry (replaces SQLite)
export const schemas = pgTable('schemas', {
  id: serial('id').primaryKey(),
  sourceName: text('source_name').notNull().unique(),
  schemaJson: jsonb('schema_json').notNull(),
  behavioralProfile: jsonb('behavioral_profile'),  // New in v2
  createdAt: timestamp('created_at').defaultNow(),
  updatedAt: timestamp('updated_at').defaultNow(),
});

// Patch registry for control loop
export const patchRegistry = pgTable('patch_registry', {
  id: serial('id').primaryKey(),
  rhythmHash: text('rhythm_hash').notNull().unique(),
  rule: text('rule').notNull(),  // 'ALLOW_LIST', 'BLOCK', etc.
  reason: text('reason'),
  createdTs: integer('created_ts'),
  isActive: boolean('is_active').default(true),
});

// Incident graph (new in v2 Phase 4)
export const incidentGraph = pgTable('incident_graph', {
  id: serial('id').primaryKey(),
  metaIncidentId: text('meta_incident_id').notNull(),
  qdrantPointId: text('qdrant_point_id').notNull(),
  linkType: text('link_type'),  // 'temporal', 'trace', 'semantic'
  confidence: integer('confidence'),
  createdAt: timestamp('created_at').defaultNow(),
});

// Evaluation metrics (new in v2 Phase 3)
export const evaluationMetrics = pgTable('evaluation_metrics', {
  id: serial('id').primaryKey(),
  timestamp: integer('timestamp').notNull(),
  precision: integer('precision'),
  recall: integer('recall'),
  f1Score: integer('f1_score'),
  scenarioName: text('scenario_name'),
});

export type Schema = typeof schemas.$inferSelect;
export type Patch = typeof patchRegistry.$inferSelect;
export type IncidentGraph = typeof incidentGraph.$inferSelect;
export type EvaluationMetric = typeof evaluationMetrics.$inferSelect;
```

### 3.2 Database Client

```typescript
// src/db/registry.ts
import { drizzle } from 'drizzle-orm/node-postgres';
import { Pool } from 'pg';
import * as schema from './schema';

const pool = new Pool({
  host: process.env.POSTGRES_HOST || 'localhost',
  port: parseInt(process.env.POSTGRES_PORT || '5432'),
  database: process.env.POSTGRES_DB || 'via_registry',
  user: process.env.POSTGRES_USER || 'via',
  password: process.env.POSTGRES_PASSWORD || 'via',
});

export const db = drizzle(pool, { schema });

// Helper functions
export async function getSchema(sourceName: string) {
  return db.query.schemas.findFirst({
    where: (schemas, { eq }) => eq(schemas.sourceName, sourceName),
  });
}

export async function saveSchema(sourceName: string, schemaJson: object, behavioralProfile?: object) {
  return db.insert(schema.schemas).values({
    sourceName,
    schemaJson,
    behavioralProfile: behavioralProfile || null,
  }).onConflictDoUpdate({
    target: schema.schemas.sourceName,
    set: { schemaJson, behavioralProfile },
  });
}
```

---  

## Phase 4: HTTP API Layer

### 4.1 Server Setup with Hono

```typescript
// src/api/server.ts
import { Hono } from 'hono';
import { logger } from 'hono/logger';
import { cors } from 'hono/cors';
import { ingestRoutes } from './routes/ingest';
import { healthRoutes } from './routes/health';
import { controlRoutes } from './routes/control';
import { schemaRoutes } from './routes/schema';

const app = new Hono();

// Middleware
app.use('*', logger());
app.use('*', cors());

// Routes
app.route('/api/v1/ingest', ingestRoutes);
app.route('/api/v1/health', healthRoutes);
app.route('/api/v1/control', controlRoutes);
app.route('/api/v1/schema', schemaRoutes);

// Global error handler
app.onError((err, c) => {
  console.error('Unhandled error:', err);
  return c.json({ error: 'Internal server error' }, 500);
});

export default app;
```

### 4.2 Ingestion Endpoint with JSONL Streaming

```typescript
// src/api/routes/ingest.ts
import { Hono } from 'hono';
import { QueueWorker } from '../../queue/worker';

const app = new Hono();

// Store worker reference (injected during startup)
declare module 'hono' {
  interface ContextVariableMap {
    worker: QueueWorker;
  }
}

app.post('/stream', async (c) => {
  const worker = c.get('worker');
  const contentType = c.req.header('content-type') || '';
  
  if (contentType.includes('application/x-ndjson') || 
      contentType.includes('application/jsonl')) {
    // Handle JSONL streaming
    const body = await c.req.text();
    const logs = Bun.JSONL.parse(body);
    
    for (const log of logs) {
      await worker.enqueue(normalizeLog(log));
    }
    
    return c.json({ 
      status: 'ok', 
      ingested: logs.length 
    });
  }
  
  // Handle regular JSON array
  const { logs } = await c.req.json();
  
  for (const log of logs) {
    await worker.enqueue(normalizeLog(log));
  }
  
  return c.json({ 
    status: 'ok', 
    ingested: logs.length 
  });
});

function normalizeLog(raw: unknown): LogRecord {
  // Normalize OTel format to internal format
  const log = raw as any;
  
  return {
    id: crypto.randomUUID(),
    timestamp: Date.now() / 1000,
    service: log.resource?.attributes?.['service.name'] || 'unknown',
    severity: log.severityText || 'INFO',
    body: log.body?.stringValue || '',
    rhythmHash: generateRhythmHash(log),
    attributes: log.attributes || {},
  };
}

function generateRhythmHash(log: any): string {
  const service = log.resource?.attributes?.['service.name'] || 'unknown';
  const severity = log.severityText || 'INFO';
  const body = log.body?.stringValue || '';
  
  // Extract template (same logic as Python version)
  const template = body
    .replace(/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/g, '*')
    .replace(/\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/g, '*')
    .replace(/\b\d+\b/g, '*');
  
  return Bun.hash.xxHash64(`${service}:${severity}:${template}`).toString(16);
}

export const ingestRoutes = app;
```

---  

## Phase 5: Main Application Entry

```typescript
// src/main.ts
import app from './api/server';
import { Tier1Engine } from './services/tier1-engine';
import { QueueWorker } from './queue/worker';
import { db } from './db/registry';
import settings from './config/settings.json5';

async function main() {
  console.log('🚀 Starting VIA v2 (Bun runtime)');
  
  // Initialize database
  console.log('📦 Connecting to PostgreSQL...');
  await db.query.schemas.findFirst();  // Test connection
  
  // Initialize Tier-1 Engine
  console.log('⚡ Initializing Tier-1 detection engine...');
  const engine = new Tier1Engine(settings);
  
  // Initialize queue worker
  console.log('📥 Starting ingestion worker...');
  const worker = new QueueWorker(engine, {
    batchSize: settings.queue.batchSize,
    flushInterval: settings.queue.flushInterval,
  });
  
  // Inject worker into Hono context
  app.use('*', async (c, next) => {
    c.set('worker', worker);
    await next();
  });
  
  // Start worker
  worker.start();
  
  // Start HTTP server
  const port = parseInt(process.env.PORT || '3000');
  console.log(`🌐 Server listening on port ${port}`);
  
  Bun.serve({
    port,
    fetch: app.fetch,
  });
  
  // Graceful shutdown
  process.on('SIGINT', async () => {
    console.log('\n🛑 Shutting down gracefully...');
    worker.stop();
    process.exit(0);
  });
}

main().catch(console.error);
```

---  

## Deployment & Build

### Single Binary Build

```toml
# bunfig.toml
[install]
cache = true

[test]
coverage = true

[build]
minify = true
sourcemap = "external"
```

```json
{
  "name": "via-v2",
  "version": "2.0.0",
  "scripts": {
    "dev": "bun --watch src/main.ts",
    "build": "bun build src/main.ts --outdir ./dist --target bun",
    "compile": "bun build src/main.ts --compile --outfile via-v2",
    "db:generate": "drizzle-kit generate",
    "db:migrate": "drizzle-kit migrate",
    "test": "bun test",
    "lint": "biome check src/",
    "format": "biome format --write src/"
  },
  "dependencies": {
    "hono": "^4.x",
    "drizzle-orm": "^0.30.x",
    "pg": "^8.x",
    "@qdrant/js-client-rest": "^1.x",
    "zod": "^3.x",
    "json5": "^2.x",      // For Bun 1.2 fallback
    "jsonl": "^1.x"       // For Bun 1.2 fallback
  },
  "devDependencies": {
    "@types/pg": "^8.x",
    "drizzle-kit": "^0.20.x",
    "@biomejs/biome": "^1.x",
    "bun-types": "latest"
  }
}
```

### Docker Deployment

#### Option 1: Bun 1.3.7+ (Recommended - Native JSON5/JSONL)

```dockerfile
# Dockerfile (Bun 1.3.7+)
FROM oven/bun:1.3.7-alpine AS builder
WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM oven/bun:1.3.7-alpine
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/config ./config
EXPOSE 3000
CMD ["bun", "dist/main.js"]
```

#### Option 2: Bun 1.2 (Fallback - Uses npm packages)

```dockerfile
# Dockerfile (Bun 1.2)
FROM oven/bun:1.2-alpine AS builder
WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM oven/bun:1.2-alpine
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/config ./config
EXPOSE 3000
CMD ["bun", "dist/main.js"]
```

#### Option 3: Latest Bun (Always gets newest features)

```dockerfile
# Dockerfile (Latest Bun)
FROM oven/bun:latest-alpine AS builder
WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM oven/bun:latest-alpine
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/config ./config
EXPOSE 3000
CMD ["bun", "dist/main.js"]
```

### Docker Compose Integration

```yaml
# docker-compose.yml
services:
  via-v2:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    environment:
      - POSTGRES_HOST=postgres
      - POSTGRES_PORT=5432
      - POSTGRES_DB=via_registry
      - POSTGRES_USER=via
      - POSTGRES_PASSWORD=via
      - QDRANT_HOST=qdrant-1
      - QDRANT_PORT=6333
    depends_on:
      - postgres
      - qdrant-1
    restart: unless-stopped

  postgres:
    image: postgres:16-alpine
    environment:
      - POSTGRES_DB=via_registry
      - POSTGRES_USER=via
      - POSTGRES_PASSWORD=via
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  qdrant-1:
    image: qdrant/qdrant:latest
    ports:
      - "6333:6333"
      - "6334:6334"
    volumes:
      - ./qdrant_data/node1:/qdrant/storage

volumes:
  postgres_data:
```

---

## Migration Checklist from Python

| Phase | Python Component | Bun Replacement | Status |
|-------|-----------------|-----------------|--------|
| 1 | FastAPI + uvicorn | Hono + Bun.serve | ✅ |
| 1 | asyncio.Queue | Custom AsyncQueue | ✅ |
| 1 | Pydantic | Zod (runtime) + TS types | ✅ |
| 1 | Simhash | Bun.hash.xxHash64 | ✅ |
| 1 | Statistics (EWMA, etc.) | Custom implementations | ✅ |
| 1 | SQLite | PostgreSQL + Drizzle | ✅ |
| 2 | Schema detection | Port logic to TS | ⏳ |
| 2 | Gradio UI | TanStack SPA (separate) | ⏳ |
| 3 | Simulation framework | Port to Bun | ⏳ |
| 4 | Incident correlation | Port logic to TS | ⏳ |

---

## Performance Expectations

| Metric | Python (Current) | Bun (Target) | Improvement |
|--------|-----------------|--------------|-------------|
| **Cold Start** | ~2s | ~100ms | 20x |
| **Ingestion (logs/sec)** | ~10K | ~50K+ | 5x |
| **Memory (idle)** | ~150MB | ~30MB | 5x |
| **Memory (Tier-1 profiles)** | ~20KB each | ~15KB each | 1.3x |
| **Binary Size** | N/A (source) | ~50MB | Single file |

---

## Next Steps

1. **Bootstrap project**: `bun init` and install dependencies
2. **Implement algorithms**: Start with `src/algorithms/`
3. **Set up database**: `docker-compose up postgres` + Drizzle migrations
4. **Build ingestion pipeline**: Queue → Worker → Tier1Engine
5. **Integrate Qdrant**: Port REST API calls from Python
6. **Add LM Studio client**: HTTP client for embedding inference
7. **Build TanStack frontend**: Separate SPA project

---  

**Estimated Timeline**: 2-3 weeks for full migration of core functionality