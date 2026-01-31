use axum::{
    body::Bytes,
    extract::{FromRequest, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use once_cell::sync::Lazy;
use prometheus::{Counter, Histogram, Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::{io::Write, thread};
use tokio::net::TcpListener;
use tracing::{info, warn};
use via_core::engine::AnomalyProfile;

// --- 1. Global Metrics ---
pub static INGEST_TOTAL: Lazy<Counter> = Lazy::new(|| {
    let c = Counter::new("via_ingest_total", "Total events ingested").unwrap();
    prometheus::register(Box::new(c.clone())).unwrap();
    c
});

pub static ANOMALY_TOTAL: Lazy<Counter> = Lazy::new(|| {
    let c = Counter::new("via_anomalies_total", "Total anomalies detected").unwrap();
    prometheus::register(Box::new(c.clone())).unwrap();
    c
});

pub static DROPPED_TOTAL: Lazy<Counter> = Lazy::new(|| {
    let c = Counter::new("via_dropped_total", "Total events dropped due to backpressure").unwrap();
    prometheus::register(Box::new(c.clone())).unwrap();
    c
});

pub static PROCESSING_LATENCY: Lazy<Histogram> = Lazy::new(|| {
    let h = Histogram::with_opts(prometheus::HistogramOpts::new(
        "via_processing_duration_seconds",
        "Histogram of processing latency",
    )).unwrap();
    prometheus::register(Box::new(h.clone())).unwrap();
    h
});

// --- 2. Data Types ---

// External API Contract
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IngestEvent {
    pub u: String, 
    pub v: f64,    
    pub t: u64,    
}

// Internal Hot-Path Contract (Zero Allocation)
#[derive(Debug, Clone, Copy)]
pub struct InternalEvent {
    pub uid_hash: u64,
    pub val: f64,
    pub ts: u64,
}

#[derive(Clone)]
struct AppState {
    shard_txs: Arc<Vec<Sender<InternalEvent>>>,
}

// --- 3. Custom SIMD-JSON Extractor ---
struct SimdJson<T>(T);

impl<T, S> FromRequest<S> for SimdJson<T>
where
    T: for<'de> Deserialize<'de> + Send,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, _state).await.map_err(|e| e.into_response())?;
        let mut bytes_vec = bytes.to_vec();
        
        let val = simd_json::from_slice::<T>(&mut bytes_vec)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid JSON").into_response())?;
            
        Ok(SimdJson(val))
    }
}

// --- 4. Shard Worker (Actor Model) ---

struct ShardWorker {
    id: usize,
    rx: Receiver<InternalEvent>,
    // CHANGED: Key is u64 hash, not String. massive memory save.
    profiles: HashMap<u64, Box<AnomalyProfile>>,
    persistence_tx: Sender<String>,
}

impl ShardWorker {
    fn spawn(id: usize, rx: Receiver<InternalEvent>, p_tx: Sender<String>) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name(format!("via-shard-{}", id))
            .spawn(move || {
                let mut worker = ShardWorker {
                    id,
                    rx,
                    profiles: HashMap::new(),
                    persistence_tx: p_tx,
                };
                worker.run();
                info!(shard = id, "Shard worker stopped.");
            })
            .expect("Failed to spawn shard thread")
    }

    fn run(&mut self) {
        info!(shard = self.id, "Shard worker active.");
        
        while let Ok(event) = self.rx.recv() {
            let timer = PROCESSING_LATENCY.start_timer();
            
            let profile = self.profiles.entry(event.uid_hash).or_insert_with(|| {
                Box::new(AnomalyProfile::new(
                    0.1, 0.05, 0.1, 60,
                    50, 0.0, 5000.0, 0.99
                ))
            });

            // Zero-copy processing
            let result = profile.process_with_hash(event.ts, event.uid_hash, event.val);

            if result.is_anomaly {
                ANOMALY_TOTAL.inc();
                
                let signal = format!(
                    "{{\"t\":{},\"h\":{},\"score\":{:.4},\"sev\":{},\"type\":{}}}
",
                    event.ts, event.uid_hash, result.anomaly_score, result.severity, result.signal_type
                );
                
                let _ = self.persistence_tx.try_send(signal);
                
                // Warn is expensive, sample it in real prod
                if result.severity > 2 {
                    warn!(
                        shard = self.id,
                        hash = event.uid_hash,
                        score = result.anomaly_score,
                        "CRITICAL ANOMALY"
                    );
                }
            }
            
            timer.observe_duration();
        }
    }
}

// --- 5. Persistence Layer ---

struct PersistenceManager;

impl PersistenceManager {
    fn spawn(rx: Receiver<String>) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("via-persistence".into())
            .spawn(move || {
                let mut current_hour = chrono::Local::now().format("%Y%m%d%H").to_string();
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(format!("anomalies_{}.log", current_hour))
                    .expect("Failed to open anomaly log");

                let mut buffer = std::io::BufWriter::with_capacity(128 * 1024, file);

                info!("Persistence manager active.");
                
                while let Ok(msg) = rx.recv() {
                    let now_hour = chrono::Local::now().format("%Y%m%d%H").to_string();
                    if now_hour != current_hour {
                        let _ = buffer.flush();
                        current_hour = now_hour;
                        let new_file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(format!("anomalies_{}.log", current_hour))
                            .expect("Failed to rotate log");
                        buffer = std::io::BufWriter::with_capacity(128 * 1024, new_file);
                    }

                    let _ = buffer.write_all(msg.as_bytes());
                }
                
                let _ = buffer.flush();
                info!("Persistence manager stopped.");
            })
            .expect("Failed to spawn persistence thread")
    }
}

// --- 7. Handlers ---

async fn ingest(
    State(state): State<AppState>,
    SimdJson(event): SimdJson<IngestEvent>,
) -> StatusCode {
    INGEST_TOTAL.inc();
    process_single_event(event, &state)
}

async fn ingest_batch(
    State(state): State<AppState>,
    SimdJson(events): SimdJson<Vec<IngestEvent>>,
) -> StatusCode {
    let count = events.len();
    INGEST_TOTAL.inc_by(count as f64);
    
    for event in events {
        // Inlining the processing logic for batch efficiency
        let hash = xxhash_rust::xxh3::xxh3_64(event.u.as_bytes());
        let shard_id = (hash as usize) % state.shard_txs.len();

        let internal = InternalEvent {
            uid_hash: hash,
            val: event.v,
            ts: event.t,
        };

        if let Err(_) = state.shard_txs[shard_id].try_send(internal) {
            DROPPED_TOTAL.inc();
        }
    }
    
    StatusCode::ACCEPTED
}

// Helper to keep DRY (though we inline in batch for speed)
#[inline(always)]
fn process_single_event(event: IngestEvent, state: &AppState) -> StatusCode {
    let hash = xxhash_rust::xxh3::xxh3_64(event.u.as_bytes());
    let shard_id = (hash as usize) % state.shard_txs.len();

    let internal = InternalEvent {
        uid_hash: hash,
        val: event.v,
        ts: event.t,
    };

    match state.shard_txs[shard_id].try_send(internal) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(_) => {
            DROPPED_TOTAL.inc();
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

// --- 7. Main ---

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 Initializing VIA Gatekeeper v2 (SOTA Zero-Copy Edition)");

    // Initialize metrics
    let _ = &*INGEST_TOTAL;
    let _ = &*ANOMALY_TOTAL;
    let _ = &*DROPPED_TOTAL;
    let _ = &*PROCESSING_LATENCY;

    let shard_count = thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
    info!(shards = shard_count, "Configuring hardware parallelism.");

    let (p_tx, p_rx) = bounded::<String>(200_000);
    let persistence_handle = PersistenceManager::spawn(p_rx);

    let mut txs = Vec::new();
    let mut worker_handles = Vec::new();

    for i in 0..shard_count {
        let (tx, rx) = bounded::<InternalEvent>(100_000);
        txs.push(tx);
        worker_handles.push(ShardWorker::spawn(i, rx, p_tx.clone()));
    }

    drop(p_tx); 

    let state = AppState {
        shard_txs: Arc::new(txs),
    };

    let app = Router::new()
        .route("/ingest", post(ingest))
        .route("/ingest/batch", post(ingest_batch))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(|| async { "OK" }))
        .with_state(state.clone());

    let addr = "0.0.0.0:3000";
    let listener = TcpListener::bind(addr).await.expect("Failed to bind port");
    
    info!(addr, "Gatekeeper listening.");
    
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install CTRL+C handler");
            info!("Shutting down... (Waiting for queues to drain)");
        })
        .await
        .expect("Server crash");

    drop(state);
    info!("Ingest channels closed.");

    for handle in worker_handles {
        handle.join().expect("Shard worker panicked");
    }
    info!("All shards drained and stopped.");

    persistence_handle.join().expect("Persistence panicked");
    info!("Persistence flushed. Goodbye.");
}