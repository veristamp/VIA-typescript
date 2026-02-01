//! Pure CPU Stress Test - Measures raw detection throughput
//!
//! This bypasses simulation and directly tests via-core detection

use std::time::Instant;
use via_core::engine::AnomalyProfile;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           VIA-CORE Pure CPU Stress Test                       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    // Test 1: Single-threaded throughput
    println!("║ Test 1: Single-threaded throughput                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let mut profile = AnomalyProfile::default();

    // Warmup
    println!("Warming up (1000 events)...");
    for i in 0..1000 {
        profile.process_with_hash(i * 1_000_000, i % 100, 100.0 + (i as f64 * 0.01));
    }

    // Benchmark single-threaded
    let events = 10_000;
    let start = Instant::now();

    let mut anomaly_count = 0u64;
    for i in 0..events {
        let signal = profile.process_with_hash(
            (1000 + i) * 1_000_000,
            (i % 1000) as u64,
            100.0 + (i as f64 * 0.05) + ((i % 500) as f64 * 0.1),
        );
        if signal.is_anomaly {
            anomaly_count += 1;
        }
    }

    let elapsed = start.elapsed();
    let eps = events as f64 / elapsed.as_secs_f64();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   Single-Thread Results                       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Events Processed:                                    {:>7} ║",
        events
    );
    println!(
        "║ Anomalies Detected:                                  {:>7} ║",
        anomaly_count
    );
    println!(
        "║ Duration:                                         {:>7.2}s ║",
        elapsed.as_secs_f64()
    );
    println!(
        "║ Throughput:                                     {:>9.0} EPS ║",
        eps
    );
    println!(
        "║ Latency per event:                              {:>9.2} µs ║",
        1_000_000.0 / eps
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Test 2: Multi-threaded throughput
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║ Test 2: Multi-threaded throughput (4 threads)                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let num_threads = 4; // Fixed for stability

    println!("Using {} threads...\n", num_threads);

    let events_per_thread = 10_000; // Reduced for faster test
    let start = Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            std::thread::spawn(move || {
                let mut profile = AnomalyProfile::default();
                let base_hash = (thread_id as u64) * 10_000_000;
                let mut anomaly_count = 0u64;

                // Warmup
                for i in 0..100 {
                    profile.process_with_hash(i * 1_000_000, base_hash + (i % 100), 100.0);
                }

                // Benchmark
                for i in 0..events_per_thread {
                    let signal = profile.process_with_hash(
                        (100 + i as u64) * 1_000_000,
                        base_hash + ((i as u64) % 1000),
                        100.0 + ((i as f64) * 0.05),
                    );
                    if signal.is_anomaly {
                        anomaly_count += 1;
                    }
                }

                anomaly_count
            })
        })
        .collect();

    let total_anomalies: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let elapsed = start.elapsed();
    let total_events = events_per_thread * num_threads;
    let eps = total_events as f64 / elapsed.as_secs_f64();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   Multi-Thread Results                        ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Threads:                                               {:>5} ║",
        num_threads
    );
    println!(
        "║ Total Events:                                        {:>7} ║",
        total_events
    );
    println!(
        "║ Anomalies Detected:                                  {:>7} ║",
        total_anomalies
    );
    println!(
        "║ Duration:                                         {:>7.2}s ║",
        elapsed.as_secs_f64()
    );
    println!(
        "║ Throughput:                                     {:>9.0} EPS ║",
        eps
    );
    println!(
        "║ Per-core throughput:                            {:>9.0} EPS ║",
        eps / num_threads as f64
    );
    println!(
        "║ Scaling efficiency:                               {:>7.1}% ║",
        (eps / num_threads as f64) / (events as f64 / elapsed.as_secs_f64())
            * 100.0
            * num_threads as f64
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Test 3: Latency distribution
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║ Test 3: Latency Distribution (10K events)                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let mut profile = AnomalyProfile::default();
    let mut latencies = Vec::with_capacity(10_000);

    // Warmup
    for i in 0..1000 {
        profile.process_with_hash(i * 1_000_000, i % 100, 100.0);
    }

    // Measure latency
    for i in 0..10_000 {
        let start = Instant::now();
        profile.process_with_hash(
            (1000 + i) * 1_000_000,
            (i % 500) as u64,
            100.0 + (i as f64 * 0.03),
        );
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p90 = latencies[latencies.len() * 90 / 100];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let avg: u64 = latencies.iter().sum::<u64>() / latencies.len() as u64;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   Latency Distribution                        ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Average:                                            {:>6} µs ║",
        avg / 1000
    );
    println!(
        "║ P50 (median):                                       {:>6} µs ║",
        p50 / 1000
    );
    println!(
        "║ P90:                                                {:>6} µs ║",
        p90 / 1000
    );
    println!(
        "║ P95:                                                {:>6} µs ║",
        p95 / 1000
    );
    println!(
        "║ P99:                                                {:>6} µs ║",
        p99 / 1000
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Summary
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                      SUMMARY                                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Max Single-Thread:                            {:>9.0} EPS ║",
        events as f64 / elapsed.as_secs_f64()
    );
    println!(
        "║ Max Multi-Thread ({} cores):                  {:>9.0} EPS ║",
        num_threads, eps
    );
    println!(
        "║ Estimated 1M EPS would need:                      {:>5} cores ║",
        (1_000_000.0 / (events as f64 / elapsed.as_secs_f64())).ceil() as usize
    );
    println!("╚══════════════════════════════════════════════════════════════╝");
}
