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

// --- 1. Global Metrics (SOTA Thread-safe counters) ---
// We use the default registry for compatibility with standard Prometheus macros
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IngestEvent {
    pub u: String, // User/Entity ID
    pub v: f64,    // Value
    pub t: u64,    // Timestamp
}

#[derive(Clone)]
struct AppState {
    shard_txs: Arc<Vec<Sender<IngestEvent>>>,
}

// --- 3. Custom SOTA SIMD-JSON Extractor ---
struct SimdJson<T>(T);

impl<S, T> FromRequest<S> for SimdJson<T>
where
    T: for<'de> Deserialize<'de>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state).await.map_err(|e| e.into_response())?;
        let mut bytes_vec = bytes.to_vec();
        
        // SIMD-JSON parsing
        let val = simd_json::from_slice::<T>(&mut bytes_vec)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid JSON").into_response())?;
            
        Ok(SimdJson(val))
    }
}

// --- 4. Shard Worker (Actor Model) ---

struct ShardWorker {
    id: usize,
    rx: Receiver<IngestEvent>,
    profiles: HashMap<String, Box<AnomalyProfile>>,
    persistence_tx: Sender<String>,
}

impl ShardWorker {
    fn spawn(id: usize, rx: Receiver<IngestEvent>, p_tx: Sender<String>) -> thread::JoinHandle<()> {
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
            
            let profile = self.profiles.entry(event.u.clone()).or_insert_with(|| {
                Box::new(AnomalyProfile::new(
                    0.1, 0.05, 0.1, 60,
                    50, 0.0, 5000.0, 0.99
                ))
            });

            let result = profile.process(event.t, &event.u, event.v);

            if result.is_anomaly {
                ANOMALY_TOTAL.inc();
                
                let signal = format!(
                    "{{\"t\":{},\"u\":\"{}\",\"score\":{:.4},\"severity\":{},\"type\":{}}}
",
                    event.t, event.u, result.anomaly_score, result.severity, result.signal_type
                );
                
                let _ = self.persistence_tx.try_send(signal);
                
                warn!(
                    shard = self.id,
                    user = event.u,
                    score = result.anomaly_score,
                    "ANOMALY"
                );
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
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(format!("anomalies_{}.log", current_hour))
                    .expect("Failed to open anomaly log");

                let mut buffer = std::io::BufWriter::with_capacity(128 * 1024, file);

                info!("Persistence manager active.");
                
                while let Ok(msg) = rx.recv() {
                    // Hourly Rotation
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
                
                // Final flush on shutdown
                let _ = buffer.flush();
                info!("Persistence manager stopped.");
            })
            .expect("Failed to spawn persistence thread")
    }
}

// --- 6. Handlers ---

async fn ingest(
    State(state): State<AppState>,
    SimdJson(event): SimdJson<IngestEvent>,
) -> StatusCode {
    INGEST_TOTAL.inc();
    
    let hash = xxhash_rust::xxh3::xxh3_64(event.u.as_bytes());
    let shard_id = (hash as usize) % state.shard_txs.len();

    match state.shard_txs[shard_id].try_send(event) {
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
    
    info!("🚀 Initializing VIA Gatekeeper v2 (SOTA Edition)");

    // Initialize metrics early to avoid race conditions in Lazy
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
        let (tx, rx) = bounded::<IngestEvent>(100_000);
        txs.push(tx);
        worker_handles.push(ShardWorker::spawn(i, rx, p_tx.clone()));
    }

    // Drop p_tx in main so only workers hold it.
    drop(p_tx); 

    let state = AppState {
        shard_txs: Arc::new(txs),
    };

    let app = Router::new()
        .route("/ingest", post(ingest))
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

    // 1. Drop Ingest Senders (State)
    // This closes the input side of the shard channels.
    drop(state);
    info!("Ingest channels closed.");

    // 2. Wait for Shard Workers to Drain
    for handle in worker_handles {
        handle.join().expect("Shard worker panicked");
    }
    info!("All shards drained and stopped.");

    // 3. Wait for Persistence to Drain
    // (Happens automatically because workers dropped their p_tx clones)
    persistence_handle.join().expect("Persistence panicked");
    info!("Persistence flushed. Goodbye.");
}
