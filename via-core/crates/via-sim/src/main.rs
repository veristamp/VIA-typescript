//! via-sim - SOTA OTel Log Simulator
//!
//! Usage:
//!   via-sim generate --duration 5m --scenario normal_traffic
//!   via-sim generate --duration 1m --anomalies memory_leak,ddos
//!   via-sim interactive --port 8080
//!   via-sim list

use clap::{Parser, Subcommand, ValueEnum};
use via_sim::{SimulationEngine, scenarios};

#[derive(Parser)]
#[command(name = "via-sim")]
#[command(about = "SOTA OTel log simulation with controlled anomaly injection")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate logs with optional anomaly injection
    Generate {
        /// Duration (e.g., 5m, 1h, 30s)
        #[arg(short, long, default_value = "1m")]
        duration: String,

        /// Base scenario for background traffic
        #[arg(short, long, default_value = "normal_traffic")]
        scenario: String,

        /// Anomalies to inject (comma-separated)
        #[arg(short, long)]
        anomalies: Option<String>,

        /// Output format
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,

        /// Tick interval in milliseconds
        #[arg(long, default_value = "100")]
        tick_ms: u64,
    },

    /// List available scenarios
    List,

    /// Interactive mode with HTTP API
    Interactive {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Run throughput benchmark
    Benchmark {
        /// Duration
        #[arg(short, long, default_value = "10s")]
        duration: String,

        /// Target events per second
        #[arg(short, long, default_value = "100000")]
        target_eps: u64,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputFormat {
    Json,
    JsonLines,
    Pretty,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            duration,
            scenario,
            anomalies,
            format,
            tick_ms,
        } => {
            run_generate(duration, scenario, anomalies, format, tick_ms);
        }
        Commands::List => {
            run_list();
        }
        Commands::Interactive { port, host } => {
            run_interactive(host, port);
        }
        Commands::Benchmark {
            duration,
            target_eps,
        } => {
            run_benchmark(duration, target_eps);
        }
    }
}

fn run_generate(
    duration: String,
    scenario: String,
    anomalies: Option<String>,
    format: OutputFormat,
    tick_ms: u64,
) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║           VIA-SIM Log Generation                             ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║ Duration: {:50} ║", duration);
    eprintln!("║ Scenario: {:50} ║", scenario);
    eprintln!(
        "║ Anomalies: {:49} ║",
        anomalies.as_deref().unwrap_or("none")
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    let duration_ns = parse_duration(&duration) * 1_000_000_000;
    let tick_ns = tick_ms * 1_000_000;

    let mut engine = SimulationEngine::new();
    engine.start(&scenario);

    // Schedule anomalies if provided
    if let Some(anomaly_list) = anomalies {
        let anomaly_count = anomaly_list.split(',').count();
        let anomaly_duration_ns = duration_ns / (anomaly_count as u64 + 1);
        let mut offset_ns = anomaly_duration_ns / 2; // Start anomalies after initial baseline

        for anomaly_name in anomaly_list.split(',') {
            let name = anomaly_name.trim();
            if let Some(id) = engine.schedule_anomaly(name, offset_ns, anomaly_duration_ns / 2) {
                eprintln!(
                    "Scheduled anomaly '{}' (id: {}) at offset {}ms for {}ms",
                    name,
                    id,
                    offset_ns / 1_000_000,
                    anomaly_duration_ns / 2 / 1_000_000
                );
            } else {
                eprintln!("Warning: Unknown anomaly type '{}'", name);
            }
            offset_ns += anomaly_duration_ns;
        }
    }

    eprintln!("\nGenerating logs...\n");

    let mut total_logs = 0u64;
    let mut total_anomaly_logs = 0u64;
    let mut elapsed_ns = 0u64;

    while elapsed_ns < duration_ns {
        let batch = engine.tick(tick_ns);
        elapsed_ns += tick_ns;

        // Output logs
        for resource_log in &batch.logs.resourceLogs {
            for scope_log in &resource_log.scopeLogs {
                for log in &scope_log.logRecords {
                    total_logs += 1;
                    if log.isGroundTruthAnomaly {
                        total_anomaly_logs += 1;
                    }

                    match format {
                        OutputFormat::Json => {
                            println!("{}", serde_json::to_string(log).unwrap());
                        }
                        OutputFormat::JsonLines => {
                            println!("{}", serde_json::to_string(log).unwrap());
                        }
                        OutputFormat::Pretty => {
                            let anomaly_marker = if log.isGroundTruthAnomaly {
                                " [ANOMALY]"
                            } else {
                                ""
                            };
                            println!(
                                "[{}] {} - {}{}",
                                log.severityText,
                                log.service_name().unwrap_or("unknown"),
                                log.body.as_str().unwrap_or(""),
                                anomaly_marker
                            );
                        }
                    }
                }
            }
        }

        // Progress update every ~5 seconds of simulated time
        if elapsed_ns % (5_000_000_000) < tick_ns {
            let progress = (elapsed_ns as f64 / duration_ns as f64) * 100.0;
            eprintln!(
                "Progress: {:.1}% | Logs: {} | Anomaly logs: {}",
                progress, total_logs, total_anomaly_logs
            );
        }
    }

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║                     Generation Complete                       ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║ Total logs generated: {:38} ║", total_logs);
    eprintln!("║ Anomaly logs (ground truth): {:31} ║", total_anomaly_logs);
    eprintln!(
        "║ Anomaly ratio: {:42.2}% ║",
        (total_anomaly_logs as f64 / total_logs.max(1) as f64) * 100.0
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}

fn run_list() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              Available Simulation Scenarios                   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for (name, description) in scenarios::list_scenarios() {
        println!("║ {:20} - {:36} ║", name, description);
    }

    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\nUsage: via-sim generate --scenario <SCENARIO> --anomalies <ANOMALY1,ANOMALY2>");
}

fn run_interactive(host: String, port: u16) {
    use via_sim::{ApiConfig, create_shared_state, print_api_docs};

    let config = ApiConfig {
        host: host.clone(),
        port,
        cors_enabled: true,
        tick_interval_ms: 100,
    };

    print_api_docs(&config);

    let _state = create_shared_state(config);

    eprintln!("Interactive HTTP server not yet implemented.");
    eprintln!("For now, use the library API directly or integrate with a web framework.");
    eprintln!("\nExample integration with axum/actix/tiny_http would go here.");
}

fn run_benchmark(duration: String, target_eps: u64) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║           VIA-SIM Throughput Benchmark                        ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║ Duration: {:50} ║", duration);
    eprintln!("║ Target EPS: {:48} ║", target_eps);
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    let duration_sec = parse_duration(&duration);

    let mut engine = SimulationEngine::new();
    engine.start("normal_traffic");

    // Add multiple scenarios for higher throughput
    engine.add_scenario_by_name("traffic_spike");
    engine.add_scenario_by_name("error_spike");

    let start = std::time::Instant::now();
    let mut total_logs = 0u64;
    let tick_ns = 10_000_000u64; // 10ms ticks for high granularity

    // Run simulation
    let sim_duration_ns = duration_sec * 1_000_000_000;
    let mut sim_elapsed_ns = 0u64;

    while sim_elapsed_ns < sim_duration_ns {
        let batch = engine.tick(tick_ns);
        sim_elapsed_ns += tick_ns;

        for resource_log in &batch.logs.resourceLogs {
            for scope_log in &resource_log.scopeLogs {
                total_logs += scope_log.logRecords.len() as u64;
            }
        }
    }

    let elapsed = start.elapsed();
    let actual_eps = total_logs as f64 / elapsed.as_secs_f64();
    let efficiency = (actual_eps / target_eps as f64) * 100.0;

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║                   Benchmark Results                           ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║ Wall clock time: {:42.2}s ║", elapsed.as_secs_f64());
    eprintln!("║ Total logs: {:48} ║", total_logs);
    eprintln!("║ Target EPS: {:48} ║", target_eps);
    eprintln!("║ Actual EPS: {:48.0} ║", actual_eps);
    eprintln!("║ Efficiency: {:47.1}% ║", efficiency);
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}

fn parse_duration(s: &str) -> u64 {
    let s = s.trim();
    if s.ends_with("m") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 60
    } else if s.ends_with("h") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 3600
    } else if s.ends_with("s") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(60)
    } else {
        s.parse::<u64>().unwrap_or(60)
    }
}
