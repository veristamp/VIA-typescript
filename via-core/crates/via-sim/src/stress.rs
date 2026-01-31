//! via-stress - Simple HTTP Load Generator for Stress Testing Gatekeeper
//!
//! This is a lightweight, high-throughput HTTP client that sends
//! random events to the gatekeeper API for raw stress testing.
//!
//! Usage:
//!   via-stress --target http://localhost:3000 --duration 60s --rate 100000
//!   via-stress --batch-size 100 --concurrency 64

use clap::Parser;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

#[derive(Parser)]
#[command(name = "via-stress")]
#[command(about = "High-throughput HTTP stress tester for VIA gatekeeper")]
struct Cli {
    /// Target URL
    #[arg(short, long, default_value = "http://127.0.0.1:3000/ingest/batch")]
    target: String,

    /// Test duration (e.g., 30s, 5m, 1h)
    #[arg(short, long, default_value = "30s")]
    duration: String,

    /// Target events per second
    #[arg(short, long, default_value = "100000")]
    rate: usize,

    /// Batch size per request
    #[arg(short, long, default_value = "50")]
    batch_size: usize,

    /// Number of concurrent connections
    #[arg(short, long, default_value = "64")]
    concurrency: usize,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    println!("🔥 VIA Stress Test - High Throughput HTTP Load Generator");
    println!("========================================================");
    println!("Target: {} (Batch Size: {})", cli.target, cli.batch_size);
    println!("Concurrency: {}", cli.concurrency);
    println!("Target Rate: {} EPS", cli.rate);
    println!("Duration: {}", cli.duration);
    println!();

    let duration_secs = parse_duration(&cli.duration);
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(cli.concurrency)
        .tcp_nodelay(true)
        .build()
        .unwrap();

    let total_events = Arc::new(AtomicUsize::new(0));
    let dropped_events = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();
    let mut handles = Vec::new();

    // Calculate batches per second per connection
    let batches_per_sec = cli.rate / (cli.batch_size * cli.concurrency);
    let sleep_duration = if batches_per_sec > 0 {
        Duration::from_millis(1000 / batches_per_sec as u64)
    } else {
        Duration::from_micros(100)
    };

    for worker_id in 0..cli.concurrency {
        let client = client.clone();
        let total_events = total_events.clone();
        let dropped_events = dropped_events.clone();
        let target_url = cli.target.clone();
        let batch_size = cli.batch_size;

        handles.push(tokio::spawn(async move {
            let mut rng = fastrand::Rng::new();
            let worker_offset = (worker_id as u32) * 1_000_000;

            loop {
                if start_time.elapsed().as_secs() >= duration_secs {
                    break;
                }

                // Generate Batch
                let mut batch = Vec::with_capacity(batch_size);
                for _ in 0..batch_size {
                    let user_id = (worker_offset + rng.u32(0..1_000_000)) as usize;
                    let value = rng.f64() * 1000.0;
                    let ts = chrono::Utc::now().timestamp() as u64;

                    batch.push(serde_json::json!({
                        "u": format!("user_{}", user_id),
                        "v": value,
                        "t": ts
                    }));
                }

                // Send Batch
                match client.post(&target_url).json(&batch).send().await {
                    Ok(resp) => {
                        if resp.status().is_success()
                            || resp.status() == reqwest::StatusCode::ACCEPTED
                        {
                            total_events.fetch_add(batch_size, Ordering::Relaxed);
                        } else {
                            dropped_events.fetch_add(batch_size, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        dropped_events.fetch_add(batch_size, Ordering::Relaxed);
                    }
                }

                // Rate limiting
                if batches_per_sec > 0 {
                    tokio::time::sleep(sleep_duration).await;
                }
            }
        }));
    }

    // Monitor Loop
    let monitor_total = total_events.clone();
    let monitor_dropped = dropped_events.clone();
    let monitor = tokio::spawn(async move {
        let mut last_count = 0;
        let monitor_start = Instant::now();
        loop {
            time::sleep(Duration::from_secs(1)).await;
            let current = monitor_total.load(Ordering::Relaxed);
            let eps = current - last_count;
            last_count = current;

            let elapsed = monitor_start.elapsed().as_secs();
            println!(
                "[{:02}s] EPS: {:<8} | Total: {} | Dropped: {}",
                elapsed,
                eps,
                current,
                monitor_dropped.load(Ordering::Relaxed)
            );

            if monitor_start.elapsed().as_secs() >= duration_secs {
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

    println!("\n=== Stress Test Results ===");
    println!("Total Successful Events: {}", total);
    println!("Total Failed/Dropped:    {}", dropped);
    println!("Actual Duration:         {:.2?}", duration);
    println!(
        "Average Throughput:      {:.0} EPS",
        total as f64 / duration.as_secs_f64()
    );
    println!(
        "Success Rate:            {:.2}%",
        (total as f64 / (total + dropped) as f64) * 100.0
    );
}

fn parse_duration(s: &str) -> u64 {
    let s = s.trim();
    if s.ends_with("m") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 60
    } else if s.ends_with("h") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 3600
    } else if s.ends_with("s") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(30)
    } else {
        s.parse::<u64>().unwrap_or(30)
    }
}
