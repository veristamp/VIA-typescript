use std::io::{self, Write};
use std::time::Instant;
use via_core::simulation::{SimulationEngine, scenarios::{traffic::NormalTraffic, performance::CpuSpike}};
use via_core::engine::AnomalyProfile;
use via_core::simulation::types::{AnyValue, LogRecord};

/// Helper to extract latency from OTel attributes
fn extract_latency(log: &LogRecord) -> Option<f64> {
    for kv in &log.attributes {
        if kv.key == "http.duration_ms" || kv.key == "latency_ms" {
            match &kv.value {
                AnyValue::Int { intValue } => return Some(*intValue as f64),
                AnyValue::Double { doubleValue } => return Some(*doubleValue),
                _ => {}
            }
        }
    }
    None
}

fn main() {
    println!("=== VIA Tier-1 Engine Benchmark ===");
    println!("Simulating ingestion pipeline with live anomaly detection...\n");

    // 1. Initialize Simulation (The World)
    let mut sim = SimulationEngine::new();
    sim.add_scenario(Box::new(NormalTraffic::new(1000.0))); // 1000 logs/sec base

    // 2. Initialize Engine (The Brain)
    // Profile: Latency Monitor
    // Alpha/Beta/Gamma=0.1 (smoothing), Period=60s
    // Hist: 50 bins, 0-2000ms range
    let mut profile = AnomalyProfile::new(
        0.1, 0.05, 0.1, 60, 
        50, 0.0, 2000.0, 0.95
    );

    let start_time = Instant::now();
    let mut total_events = 0;
    let mut detected_anomalies = 0;
    
    // Simulation Parameters
    let tick_ms = 100; // 100ms simulation steps
    let total_duration_s = 120; // Run for 2 minutes
    let attack_start_s = 40;
    let attack_end_s = 60;
    let mut attack_active = false;

    println!("{:<10} | {:<10} | {:<10} | {:<15} | {:<10}", 
        "Time (s)", "Events", "PPS", "Avg Latency", "Status");
    println!("{}", "-".repeat(65));

    for t in 0..(total_duration_s * (1000 / tick_ms)) {
        let current_sim_time_s = t as f64 * (tick_ms as f64 / 1000.0);

        // --- Dynamic Attack Injection ---
        if current_sim_time_s >= attack_start_s as f64 && !attack_active {
            // Inject CPU Spike (causes latency)
            sim.add_scenario(Box::new(CpuSpike::new("payment-service", 0.8))); 
            attack_active = true;
        }
        
        // Resetting scenarios is complex in this simple harness, 
        // so we just let the spike mix in or "stop" by ignoring it conceptually,
        // but physically removing it from the engine requires a clear_scenarios call.
        // For this benchmark, we'll let it run to see if the engine adapts or stays alarmed.
        if current_sim_time_s >= attack_end_s as f64 && attack_active {
             sim.clear_scenarios();
             sim.add_scenario(Box::new(NormalTraffic::new(1000.0)));
             attack_active = false;
        }

        // --- Tick Simulation ---
        let logs_json = sim.tick(tick_ms as u64 * 1_000_000); // ns
        
        // "Ingest" logs
        let mut batch_latency_sum = 0.0;
        let mut batch_count = 0;
        let mut batch_anomalies = 0;

        for record in logs_json.resourceLogs.iter()
            .flat_map(|rl| rl.scopeLogs.iter())
            .flat_map(|sl| sl.logRecords.iter()) 
        {
            total_events += 1;
            
            // Extract Feature: Latency
            if let Some(latency) = extract_latency(record) {
                batch_latency_sum += latency;
                batch_count += 1;

                // Process Event
                let result = profile.process(
                    t as u64 * tick_ms, // timestamp
                    &record.traceId,    // unique_id
                    latency             // value
                );

                if result.is_anomaly {
                    detected_anomalies += 1;
                    batch_anomalies += 1;
                }
            }
        }

        // --- Live Reporting (Every 1 second of sim time) ---
        if t % (1000 / tick_ms) == 0 {
            let avg_lat = if batch_count > 0 { batch_latency_sum / batch_count as f64 } else { 0.0 };
            let status = if batch_anomalies > 50 { 
                "\x1b[31mCRITICAL\x1b[0m" // Red
            } else if batch_anomalies > 5 { 
                "\x1b[33mWARNING\x1b[0m" // Yellow
            } else { 
                "\x1b[32mOK\x1b[0m" // Green
            };

            print!("\r{:<10.1} | {:<10} | {:<10} | {:<15.2} | {:<10}", 
                current_sim_time_s, total_events, batch_count * (1000/tick_ms) as usize, avg_lat, status);
            io::stdout().flush().unwrap();
        }
    }

    let elapsed = start_time.elapsed();
    println!("\n\n=== Benchmark Complete ===");
    println!("Total Events Processed: {}", total_events);
    println!("Total Anomalies Found:  {}", detected_anomalies);
    println!("Real Execution Time:    {:?}", elapsed);
    println!("Throughput:             {:.0} events/sec", total_events as f64 / elapsed.as_secs_f64());
}