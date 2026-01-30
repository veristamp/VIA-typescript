import type { QdrantService } from "./qdrant-service";
import type { ForensicAnalysisService } from "./forensic-analysis-service";

export interface AnomalySignal {
    t: number;      // Timestamp
    u: string;      // User ID / Identity
    score: number;  // Anomaly Score
    severity: number;
    type: number;   // 1=Vol, 2=Dist, 3=Card, 4=Burst
}

export class Tier2Service {
    constructor(
        private qdrant: QdrantService,
        private forensic: ForensicAnalysisService
    ) {}

    async processAnomalyBatch(signals: AnomalySignal[]) {
        if (!signals || signals.length === 0) return;

        console.log(`[Tier-2] Processing ${signals.length} anomalies...`);
        
        // 1. Transform Signals to Tier-2 Events (Vector-ready)
        const events = signals.map(sig => {
            const context = `Anomaly type ${sig.type} for user ${sig.u} with score ${sig.score.toFixed(2)}`;
            return {
                textForEmbedding: context,
                payload: {
                    entity_type: "anomaly",
                    timestamp: sig.t,
                    user_id: sig.u,
                    score: sig.score,
                    severity: sig.severity,
                    signal_type: sig.type,
                    context: context
                }
            };
        });

        // 2. Store in Qdrant (Persistent Memory)
        await this.qdrant.ingestToTier2(events);

        // 3. Trigger Forensic Analysis (Async)
        // Check if this new batch correlates with recent incidents
        const endTs = Date.now() / 1000;
        const startTs = endTs - 3600; // Last hour
        await this.forensic.correlateIncidents(startTs, endTs);
    }
}
