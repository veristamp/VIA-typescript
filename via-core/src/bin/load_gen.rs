use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

// --- Config ---
const BATCH_URL: &str = "http://127.0.0.1:3000/ingest/batch";
const CONCURRENCY: usize = 64; 
const DURATION_SECS: u64 = 30; 
const BATCH_SIZE: usize = 50; 

#[tokio::main]
async fn main() {
    println!("🔥 VIA SOTA Load Generator (Hammer v2.1) initializing...");
    println!("Target: {} (Batch Size: {})", BATCH_URL, BATCH_SIZE);
    println!("Concurrency: {}", CONCURRENCY);

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(CONCURRENCY)
        .tcp_nodelay(true)
        .build()
        .unwrap();

    let total_events = Arc::new(AtomicUsize::new(0)); 
    let dropped_events = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..CONCURRENCY {
        let client = client.clone();
        let total_events = total_events.clone();
        let dropped_events = dropped_events.clone();
        
        handles.push(tokio::spawn(async move {
            loop {
                if start_time.elapsed().as_secs() >= DURATION_SECS {
                    break;
                }

                // Generate Batch using FASTRAND (High Performance)
                let mut batch = Vec::with_capacity(BATCH_SIZE);
                for _ in 0..BATCH_SIZE {
                    let user_id = fastrand::u32(0..1_000_000);
                    let value = fastrand::f64() * 1000.0;
                    let ts = chrono::Utc::now().timestamp() as u64;

                    batch.push(serde_json::json!({
                        "u": format!("user_{}", user_id),
                        "v": value,
                        "t": ts
                    }));
                }

                // Send Batch
                match client.post(BATCH_URL)
                    .json(&batch)
                    .send()
                    .await {
                    Ok(resp) => {
                        if resp.status().is_success() || resp.status() == reqwest::StatusCode::ACCEPTED {
                            total_events.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                        } else {
                            dropped_events.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        dropped_events.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    // Monitor Loop
    let monitor_total = total_events.clone();
    let monitor = tokio::spawn(async move {
        let mut last_count = 0;
        let monitor_start = Instant::now();
        loop {
            time::sleep(Duration::from_secs(1)).await;
            let current = monitor_total.load(Ordering::Relaxed);
            let eps = current - last_count; 
            last_count = current;
            
            let elapsed = monitor_start.elapsed().as_secs();
            println!("[{:02}s] EPS: {:<8} | Total: {}", elapsed, eps, current);
            
            if monitor_start.elapsed().as_secs() >= DURATION_SECS {
                break;
            }
        }
    });

    for h in handles {
        let _ = h.await;
    }
    let _ = monitor.await;

    let duration = start_time.elapsed();
    let total = total_events.load(Ordering::SeqCst);
    let dropped = dropped_events.load(Ordering::SeqCst);
    
    println!("\n=== Final Benchmark Report (Batch Mode) ===");
    println!("Total Successful Events: {}", total);
    println!("Total Failed/Dropped:    {}", dropped);
    println!("Actual Duration:         {:.2?}", duration);
    println!("Average Throughput:      {:.0} EPS", total as f64 / duration.as_secs_f64());
    println!("Success Rate:            {:.2}%", (total as f64 / (total + dropped) as f64) * 100.0);
}
