//! via-sim - High-Scale OTel Log Simulator
//!
//! Usage:
//!   via-sim microservices --duration 5m --anomalies credential_stuffing,memory_leak
//!   via-sim data-pipeline --scale 10.0 --output kafka://localhost:9092
//!   via-sim throughput-test --duration 1m --format json

use clap::{Parser, Subcommand, ValueEnum};
use via_sim::generator::{topologies, AnomalyConfig, AnomalyType, LogGenerator};
#[derive(Parser)]
#[command(name = "via-sim")]
#[command(about = "Enterprise-scale OTel log generator for testing and benchmarking")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate microservices logs
    Microservices {
        /// Duration (e.g., 5m, 1h)
        #[arg(short, long, default_value = "5m")]
        duration: String,

        /// Event rate multiplier
        #[arg(short, long, default_value = "1.0")]
        scale: f64,

        /// Anomalies to inject (comma-separated)
        #[arg(short, long)]
        anomalies: Option<String>,

        /// Output format
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,

        /// Output target
        #[arg(short, long, default_value = "stdout")]
        output: String,
    },

    /// Generate data pipeline logs
    DataPipeline {
        /// Duration
        #[arg(short, long, default_value = "5m")]
        duration: String,

        /// Scale factor
        #[arg(short, long, default_value = "1.0")]
        scale: f64,

        /// Anomalies
        #[arg(short, long)]
        anomalies: Option<String>,

        /// Output format
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,
    },

    /// Generate infrastructure logs
    Infrastructure {
        /// Duration
        #[arg(short, long, default_value = "5m")]
        duration: String,

        /// Scale factor
        #[arg(short, long, default_value = "1.0")]
        scale: f64,

        /// Output format
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,
    },

    /// Pure throughput test
    ThroughputTest {
        /// Duration
        #[arg(short, long, default_value = "1m")]
        duration: String,

        /// Target EPS
        #[arg(short, long, default_value = "1000000")]
        target_eps: u64,
    },

    /// Interactive mode with real-time control
    Interactive {
        /// Web UI port
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputFormat {
    Json,
    JsonLines,
    Csv,
    OtelProto,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Microservices {
            duration,
            scale,
            anomalies,
            format,
            output,
        } => {
            run_microservices(duration, scale, anomalies, format, output);
        }
        Commands::DataPipeline {
            duration,
            scale,
            anomalies,
            format,
        } => {
            run_data_pipeline(duration, scale, anomalies, format);
        }
        Commands::Infrastructure {
            duration,
            scale,
            format,
        } => {
            run_infrastructure(duration, scale, format);
        }
        Commands::ThroughputTest {
            duration,
            target_eps,
        } => {
            run_throughput_test(duration, target_eps);
        }
        Commands::Interactive { port } => {
            run_interactive(port);
        }
    }
}

fn run_microservices(
    duration: String,
    scale: f64,
    anomalies: Option<String>,
    _format: OutputFormat,
    _output: String,
) {
    println!("Starting microservices simulation...");
    println!("Duration: {}, Scale: {}", duration, scale);

    let duration_sec = parse_duration(&duration);
    let mut generator = LogGenerator::new(topologies::microservices());

    // Apply scale to all services
    if scale != 1.0 {
        for service in &mut generator.services {
            service.base_rps *= scale;
        }
    }

    // Inject anomalies
    if let Some(anomaly_list) = anomalies {
        for anomaly_type in anomaly_list.split(',') {
            let anomaly = parse_anomaly(anomaly_type.trim());
            println!("Injected: {:?}", anomaly.anomaly_type);
            generator.inject_anomaly(anomaly);
        }
    }

    // Generate logs
    let window_sec = 10u64;
    let windows = duration_sec / window_sec;

    for window in 0..windows {
        let logs = generator.generate_window(window_sec);

        // Output as JSON
        for log in logs {
            println!("{}", serde_json::to_string(&log).unwrap());
        }

        if window % 10 == 0 {
            let stats = generator.get_stats();
            eprintln!(
                "Progress: {}/{} windows, {} total events",
                window, windows, stats.total_events
            );
        }
    }

    let final_stats = generator.get_stats();
    eprintln!("\nSimulation complete!");
    eprintln!("Total events generated: {}", final_stats.total_events);
    eprintln!(
        "Average EPS: {}",
        final_stats.total_events as f64 / duration_sec as f64
    );
}

fn run_data_pipeline(
    duration: String,
    scale: f64,
    anomalies: Option<String>,
    _format: OutputFormat,
) {
    println!("Starting data pipeline simulation...");
    let duration_sec = parse_duration(&duration);
    let mut generator = LogGenerator::new(topologies::data_pipeline());

    if scale != 1.0 {
        for service in &mut generator.services {
            service.base_rps *= scale;
        }
    }

    if let Some(anomaly_list) = anomalies {
        for anomaly_type in anomaly_list.split(',') {
            let anomaly = parse_anomaly(anomaly_type.trim());
            generator.inject_anomaly(anomaly);
        }
    }

    let logs = generator.generate_window(duration_sec);
    for log in logs {
        println!("{}", serde_json::to_string(&log).unwrap());
    }
}

fn run_infrastructure(duration: String, scale: f64, _format: OutputFormat) {
    println!("Starting infrastructure simulation...");
    let duration_sec = parse_duration(&duration);
    let mut generator = LogGenerator::new(topologies::infrastructure());

    if scale != 1.0 {
        for service in &mut generator.services {
            service.base_rps *= scale;
        }
    }

    let logs = generator.generate_window(duration_sec);
    for log in logs {
        println!("{}", serde_json::to_string(&log).unwrap());
    }
}

fn run_throughput_test(duration: String, target_eps: u64) {
    println!("Running throughput test...");
    println!("Target: {} EPS for {}", target_eps, duration);

    let duration_sec = parse_duration(&duration);
    let mut generator = LogGenerator::new(topologies::infrastructure());

    // Scale to achieve target EPS
    let current_total_rps: f64 = generator.services.iter().map(|s| s.base_rps).sum();
    let scale = target_eps as f64 / current_total_rps;

    for service in &mut generator.services {
        service.base_rps *= scale;
    }

    let start = std::time::Instant::now();
    let logs = generator.generate_window(duration_sec);
    let elapsed = start.elapsed();

    let actual_eps = logs.len() as f64 / elapsed.as_secs_f64();
    println!("\nThroughput test results:");
    println!("  Target EPS: {}", target_eps);
    println!("  Actual EPS: {:.0}", actual_eps);
    println!(
        "  Efficiency: {:.1}%",
        (actual_eps / target_eps as f64) * 100.0
    );
}

fn run_interactive(port: u16) {
    println!("Starting interactive mode on port {}", port);
    println!("Web UI: http://localhost:{}", port);
    println!("API: http://localhost:{}/api", port);

    // This would start a web server with real-time control
    // For now, just a placeholder
    println!("Interactive mode not yet implemented. Use CLI mode instead.");
}

fn parse_duration(s: &str) -> u64 {
    let s = s.trim();
    if s.ends_with("m") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(5) * 60
    } else if s.ends_with("h") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 3600
    } else if s.ends_with("s") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(300)
    } else {
        s.parse::<u64>().unwrap_or(300)
    }
}

fn parse_anomaly(s: &str) -> AnomalyConfig {
    match s {
        "credential_stuffing" => AnomalyConfig {
            anomaly_type: AnomalyType::CredentialStuffing {
                attempts_per_sec: 500.0,
            },
            start_time_sec: 0,
            duration_sec: 60,
            severity: 0.9,
            target_services: vec!["auth-service".to_string()],
        },
        "memory_leak" => AnomalyConfig {
            anomaly_type: AnomalyType::MemoryLeak {
                leak_rate_mb_per_sec: 50.0,
            },
            start_time_sec: 0,
            duration_sec: 300,
            severity: 0.85,
            target_services: vec!["payment-service".to_string()],
        },
        "traffic_spike" => AnomalyConfig {
            anomaly_type: AnomalyType::TrafficSpike { multiplier: 10.0 },
            start_time_sec: 0,
            duration_sec: 30,
            severity: 0.8,
            target_services: vec!["api-gateway".to_string()],
        },
        "ddos" => AnomalyConfig {
            anomaly_type: AnomalyType::DDoSAttack { source_ips: 10000 },
            start_time_sec: 0,
            duration_sec: 60,
            severity: 0.95,
            target_services: vec!["api-gateway".to_string()],
        },
        "sql_injection" => AnomalyConfig {
            anomaly_type: AnomalyType::SqlInjection { probe_rate: 100.0 },
            start_time_sec: 0,
            duration_sec: 120,
            severity: 0.85,
            target_services: vec!["inventory-service".to_string()],
        },
        _ => AnomalyConfig {
            anomaly_type: AnomalyType::TrafficSpike { multiplier: 5.0 },
            start_time_sec: 0,
            duration_sec: 60,
            severity: 0.7,
            target_services: vec!["api-gateway".to_string()],
        },
    }
}
