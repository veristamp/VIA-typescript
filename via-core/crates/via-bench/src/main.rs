//! via-bench - Benchmark Suite for VIA Detection
//!
//! Usage:
//!   via-bench run-all                    # Run all benchmark scenarios
//!   via-bench mixed-workload             # Run mixed anomaly test
//!   via-bench security-audit             # Run security-focused test
//!   via-bench performance-stress         # Run performance test
//!   via-bench throughput                 # Maximum throughput test
//!   via-bench compare results1.json results2.json  # Compare results

use clap::{Parser, Subcommand};
use via_bench::{BenchmarkConfig, BenchmarkRunner, scenarios};

#[derive(Parser)]
#[command(name = "via-bench")]
#[command(about = "Comprehensive benchmark suite for VIA anomaly detection")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output file for results
    #[arg(short, long, global = true)]
    output: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Batch size for batch processing mode (0 = single event, default)
    #[arg(short, long, global = true, default_value = "0")]
    batch: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all benchmark scenarios
    RunAll {
        /// Export format
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Run mixed workload benchmark
    MixedWorkload {
        /// Duration override (minutes)
        #[arg(short, long)]
        duration: Option<u64>,
    },

    /// Run security audit benchmark
    SecurityAudit,

    /// Run performance stress test
    PerformanceStress,

    /// Run maximum throughput test
    Throughput {
        /// Duration in minutes
        #[arg(short, long, default_value = "2")]
        duration: u64,
    },

    /// Quick validation test
    Quick,

    /// Compare benchmark results
    Compare {
        /// Result files to compare
        files: Vec<String>,

        /// Output comparison to file
        #[arg(short, long)]
        output: Option<String>,
    },

    /// List available detectors
    ListDetectors,

    /// Export results in various formats
    Export {
        /// Input result file
        input: String,

        /// Output format
        #[arg(short, long, default_value = "html")]
        format: String,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let batch_size = cli.batch;

    match cli.command {
        Commands::RunAll { format } => {
            run_all_benchmarks(&format, cli.output, cli.verbose, batch_size);
        }
        Commands::MixedWorkload { duration } => {
            run_single_benchmark("mixed", duration, cli.output, batch_size);
        }
        Commands::SecurityAudit => {
            run_single_benchmark("security", None, cli.output, batch_size);
        }
        Commands::PerformanceStress => {
            run_single_benchmark("performance", None, cli.output, batch_size);
        }
        Commands::Throughput { duration } => {
            run_throughput_benchmark(duration, cli.output, batch_size);
        }
        Commands::Quick => {
            run_single_benchmark("quick", None, cli.output, batch_size);
        }
        Commands::Compare { files, output } => {
            compare_results(&files, output);
        }
        Commands::ListDetectors => {
            list_detectors();
        }
        Commands::Export {
            input,
            format,
            output,
        } => {
            export_results(&input, &format, output);
        }
    }
}

fn run_all_benchmarks(format: &str, output: Option<String>, verbose: bool, batch_size: usize) {
    println!(
        "Running all benchmarks... (batch_size: {})\n",
        if batch_size > 0 {
            format!("{}", batch_size)
        } else {
            "single".to_string()
        }
    );

    let configs: Vec<BenchmarkConfig> = vec![
        scenarios::mixed_workload(),
        scenarios::security_audit(),
        scenarios::performance_stress(),
        scenarios::throughput_test(),
    ]
    .into_iter()
    .map(|mut c| {
        c.batch_size = batch_size;
        c
    })
    .collect();

    let mut all_results = Vec::new();

    for config in configs {
        if verbose {
            println!("Running: {}", config.name);
        }

        let mut runner = BenchmarkRunner::new();
        let results = runner.run(config);

        if verbose {
            runner.print_results(&results);
            println!();
        }

        all_results.push(results);
    }

    // Export results
    let json = serde_json::to_string_pretty(&all_results).unwrap();

    if let Some(output_file) = output {
        std::fs::write(&output_file, json).expect("Failed to write results");
        println!("Results saved to: {}", output_file);
    } else {
        match format {
            "json" => println!("{}", json),
            _ => println!("Results generated ({} scenarios)", all_results.len()),
        }
    }
}

fn run_single_benchmark(
    name: &str,
    duration_override: Option<u64>,
    output: Option<String>,
    batch_size: usize,
) {
    let mut config = match name {
        "mixed" => scenarios::mixed_workload(),
        "security" => scenarios::security_audit(),
        "performance" => scenarios::performance_stress(),
        "quick" => scenarios::quick_validation(),
        _ => scenarios::mixed_workload(),
    };

    // Apply batch_size
    config.batch_size = batch_size;

    // Apply duration override if specified
    let config = if let Some(duration) = duration_override {
        BenchmarkConfig {
            duration_minutes: duration,
            ..config
        }
    } else {
        config
    };

    println!(
        "Running benchmark: {} (batch_size: {})\n",
        config.name,
        if batch_size > 0 {
            format!("{}", batch_size)
        } else {
            "single".to_string()
        }
    );

    let mut runner = BenchmarkRunner::new();
    let results = runner.run(config);
    runner.print_results(&results);

    if let Some(output_file) = output {
        let json = serde_json::to_string_pretty(&results).unwrap();
        std::fs::write(&output_file, json).expect("Failed to write results");
        println!("\nResults saved to: {}", output_file);
    }
}

fn run_throughput_benchmark(duration: u64, output: Option<String>, batch_size: usize) {
    println!(
        "Running throughput test ({} minutes, batch_size: {})...\n",
        duration,
        if batch_size > 0 {
            format!("{}", batch_size)
        } else {
            "single".to_string()
        }
    );

    let config = BenchmarkConfig {
        name: "Throughput Test".to_string(),
        base_scenario: "normal_traffic".to_string(),
        duration_minutes: duration,
        tick_ms: 10, // Small tick for high throughput
        anomalies: vec![],
        batch_size,
    };

    let mut runner = BenchmarkRunner::new();
    let results = runner.run(config);
    runner.print_results(&results);

    if let Some(output_file) = output {
        let json = serde_json::to_string_pretty(&results).unwrap();
        std::fs::write(&output_file, json).expect("Failed to write results");
    }
}

fn compare_results(files: &[String], output: Option<String>) {
    println!("Comparing {} benchmark results...\n", files.len());

    for (i, file) in files.iter().enumerate() {
        println!("{}. {}", i + 1, file);
    }

    println!("\nComparison feature not yet implemented.");
    println!("Load the JSON results and compare metrics manually.");

    if let Some(output_file) = output {
        println!("Comparison would be saved to: {}", output_file);
    }
}

fn list_detectors() {
    println!("Available SOTA Detectors:");
    println!();

    let detectors = vec![
        (
            "Volume/RPS",
            "Holt-Winters forecasting for request rate anomalies",
        ),
        (
            "Distribution/Latency",
            "Fading histogram for latency distribution shifts",
        ),
        (
            "Cardinality/Velocity",
            "HyperLogLog for new entity velocity detection",
        ),
        ("Burst/IAT", "Inter-arrival time analysis for micro-bursts"),
        (
            "Spectral/FFT",
            "Fast Fourier Transform for frequency-domain anomalies",
        ),
        (
            "ChangePoint/Trend",
            "CUSUM for trend and level shift detection",
        ),
        (
            "RRCF/Multivariate",
            "Robust Random Cut Forest for multi-dimensional outliers",
        ),
        (
            "MultiScale/Temporal",
            "Multi-resolution analysis (second/minute/hour/day)",
        ),
        ("Behavioral/Fingerprint", "Per-entity behavioral profiling"),
        (
            "Drift/Concept",
            "ADWIN and Page-Hinkley for distribution drift",
        ),
    ];

    for (i, (name, desc)) in detectors.iter().enumerate() {
        println!("{:2}. {:25} - {}", i + 1, name, desc);
    }

    println!();
    println!("Use 'via-bench <scenario>' to run a benchmark.");
}

fn export_results(input: &str, format: &str, output: Option<String>) {
    println!("Exporting {} to {} format", input, format);

    // Load results
    let content = std::fs::read_to_string(input).expect("Failed to read input file");
    let results: via_bench::BenchmarkResults =
        serde_json::from_str(&content).expect("Failed to parse results");

    match format {
        "html" => {
            let html = generate_html_report(&results);
            if let Some(output_file) = output {
                std::fs::write(&output_file, html).expect("Failed to write HTML");
                println!("HTML report saved to: {}", output_file);
            } else {
                println!("{}", html);
            }
        }
        "csv" => {
            let csv = generate_csv_report(&results);
            if let Some(output_file) = output {
                std::fs::write(&output_file, csv).expect("Failed to write CSV");
                println!("CSV report saved to: {}", output_file);
            } else {
                println!("{}", csv);
            }
        }
        _ => {
            println!("Unsupported format: {}", format);
        }
    }
}

fn generate_html_report(results: &via_bench::BenchmarkResults) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>VIA Benchmark Results</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        h1 {{ color: #333; }}
        .metric {{ margin: 20px 0; padding: 15px; background: #f5f5f5; border-radius: 5px; }}
        .metric-label {{ font-weight: bold; color: #666; }}
        .metric-value {{ font-size: 24px; color: #2196F3; }}
        table {{ width: 100%; border-collapse: collapse; margin-top: 20px; }}
        th, td {{ padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background-color: #2196F3; color: white; }}
    </style>
</head>
<body>
    <h1>VIA Detection Benchmark Results</h1>
    
    <div class="metric">
        <div class="metric-label">Total Events</div>
        <div class="metric-value">{}</div>
    </div>
    
    <div class="metric">
        <div class="metric-label">Throughput</div>
        <div class="metric-value">{:.0} EPS</div>
    </div>
    
    <div class="metric">
        <div class="metric-label">P99 Latency</div>
        <div class="metric-value">{:.2} μs</div>
    </div>
    
    <h2>Detector Performance</h2>
    <table>
        <tr>
            <th>Detector</th>
            <th>Precision</th>
            <th>Recall</th>
            <th>F1-Score</th>
        </tr>
{}    </table>
</body>
</html>"#,
        results.total_events,
        results.throughput_eps,
        results.latency_micros.p99_micros,
        results
            .detector_metrics
            .iter()
            .map(|(name, m)| {
                format!(
                    "        <tr><td>{}</td><td>{:.1}%</td><td>{:.1}%</td><td>{:.2}</td></tr>\n",
                    name,
                    m.precision * 100.0,
                    m.recall * 100.0,
                    m.f1_score
                )
            })
            .collect::<String>()
    )
}

fn generate_csv_report(results: &via_bench::BenchmarkResults) -> String {
    let mut csv = String::new();
    csv.push_str("Metric,Value\n");
    csv.push_str(&format!("Total Events,{}\n", results.total_events));
    csv.push_str(&format!("Throughput EPS,{:.0}\n", results.throughput_eps));
    csv.push_str(&format!(
        "Avg Latency μs,{:.2}\n",
        results.latency_micros.avg_micros
    ));
    csv.push_str(&format!(
        "P50 Latency μs,{:.2}\n",
        results.latency_micros.p50_micros
    ));
    csv.push_str(&format!(
        "P95 Latency μs,{:.2}\n",
        results.latency_micros.p95_micros
    ));
    csv.push_str(&format!(
        "P99 Latency μs,{:.2}\n",
        results.latency_micros.p99_micros
    ));
    csv.push_str(&format!("Precision,{:.4}\n", results.precision));
    csv.push_str(&format!("Recall,{:.4}\n", results.recall));
    csv.push_str(&format!("F1-Score,{:.4}\n", results.f1_score));

    csv.push_str("\nDetector,TP,FP,TN,FN,Precision,Recall,F1\n");
    for (name, m) in &results.detector_metrics {
        csv.push_str(&format!(
            "{},{},{},{},{},{:.4},{:.4},{:.4}\n",
            name,
            m.true_positives,
            m.false_positives,
            m.true_negatives,
            m.false_negatives,
            m.precision,
            m.recall,
            m.f1_score
        ));
    }

    csv
}
