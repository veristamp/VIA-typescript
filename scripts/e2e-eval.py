#!/usr/bin/env python3
"""
VIA End-to-End Evaluation Script (SOTA Edition)

Evaluates the full VIA pipeline (Tier-1 + Tier-2) using deterministic 
simulations from via-bench.

Usage:
    python scripts/e2e-eval.py --scenario mixed
    python scripts/e2e-eval.py --scenario security
    python scripts/e2e-eval.py --duration 5 --scenario quick
"""

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Dict, Any

import requests


@dataclass
class EvalConfig:
    scenario: str = "quick"
    duration_minutes: Optional[int] = None
    verbose: bool = False
    tier2_url: str = "http://127.0.0.1:3000"
    db_host: str = "localhost"
    db_port: int = 5432
    db_name: str = "via_registry"
    db_user: str = "via"
    db_password: str = "via"
    seed: int = 42


class Colors:
    GREEN = "\033[92m"
    RED = "\033[91m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    MAGENTA = "\033[95m"
    CYAN = "\033[96m"
    BOLD = "\033[1m"
    UNDERLINE = "\033[4m"
    END = "\033[0m"


def log(msg: str, color: str = "", verbose: bool = False, config: Optional[EvalConfig] = None):
    if verbose and not (config and config.verbose):
        return
    if color:
        print(f"{color}{msg}{Colors.END}")
    else:
        print(msg)


def wait_for_url(url: str, timeout: int = 90, name: str = "service") -> bool:
    """Wait for a URL to become available."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = requests.get(url, timeout=5)
            if 200 <= resp.status_code < 300:
                log(f"{name} is ready", Colors.GREEN)
                return True
        except requests.exceptions.RequestException:
            pass
        time.sleep(1.0)
    log(f"Timeout waiting for {name} at {url}", Colors.RED)
    return False


def reset_tier2_tables(config: EvalConfig) -> bool:
    """Reset Tier-2 database tables and Qdrant."""
    log("Resetting Tier-2 environment...", Colors.YELLOW)

    # Clear Qdrant collection
    try:
        # Check if Qdrant is up
        coll_resp = requests.get("http://localhost:6333/collections", timeout=2)
        if coll_resp.status_code == 200:
            collections = coll_resp.json().get("result", {}).get("collections", [])
            for coll in collections:
                name = coll.get("name", "")
                if name.startswith("via_"):
                    requests.delete(f"http://localhost:6333/collections/{name}", timeout=5)
            log("Qdrant collections cleared", Colors.GREEN)
    except Exception as e:
        log(f"Qdrant cleanup warning: {e}", Colors.YELLOW)

    sql = """
    TRUNCATE TABLE tier2_decisions, tier2_incidents, tier2_dead_letters, evaluation_metrics RESTART IDENTITY;
    """

    env = os.environ.copy()
    env["PGPASSWORD"] = config.db_password

    try:
        result = subprocess.run(
            [
                "psql",
                "-h", config.db_host,
                "-p", str(config.db_port),
                "-U", config.db_user,
                "-d", config.db_name,
                "-c", sql
            ],
            env=env,
            capture_output=True,
            text=True,
            timeout=30
        )
        if result.returncode == 0:
            log("PostgreSQL tables truncated", Colors.GREEN)
            return True
        else:
            log(f"Failed to reset tables: {result.stderr}", Colors.RED)
            return False
    except Exception as e:
        log(f"Database reset error: {e}", Colors.YELLOW)
        return True


def start_tier2(config: EvalConfig) -> Optional[subprocess.Popen]:
    """Start Tier-2 (Bun) server."""
    log("Starting Tier-2 (Bun)...", Colors.YELLOW)

    root_path = Path(__file__).parent.parent
    try:
        # Ensure dependencies are installed if needed, but assume they are for eval
        proc = subprocess.Popen(
            ["bun", "run", "src/main.ts"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=root_path,
            env={**os.environ, "LOG_LEVEL": "info"}
        )
        return proc
    except Exception as e:
        log(f"Failed to start Tier-2: {e}", Colors.RED)
        return None


def run_via_bench_pipeline(config: EvalConfig) -> Optional[Dict[str, Any]]:
    """Run via-bench pipeline command and capture JSON results."""
    log(f"Running via-bench pipeline [scenario={config.scenario}]...", Colors.MAGENTA)
    
    via_core_path = Path(__file__).parent.parent / "via-core"
    bench_bin = via_core_path / "target" / "release" / "via-bench"
    
    if not bench_bin.exists():
        log(f"via-bench binary not found. Run 'cargo build --release -p via-bench' first.", Colors.RED)
        return None

    cmd = [
        str(bench_bin),
        "--seed", str(config.seed),
        "pipeline",
        "--scenario", config.scenario,
        "--tier2-url", config.tier2_url,
        "--send-batch", "256"
    ]
    
    if config.duration_minutes:
        cmd.extend(["--duration", str(config.duration_minutes)])

    try:
        result = subprocess.run(
            cmd,
            cwd=via_core_path,
            capture_output=True,
            text=True,
            timeout=1200 # 20 minutes max for long benchmarks
        )
        
        if result.returncode != 0:
            log(f"via-bench failed (exit {result.returncode}):", Colors.RED)
            print(result.stderr)
            return None
            
        # Parse JSON from stdout
        # via-bench might print some text before the JSON
        output = result.stdout
        json_start = output.find('{')
        if json_start == -1:
            log("Could not find JSON in via-bench output", Colors.RED)
            print(output)
            return None
            
        return json.loads(output[json_start:])
    except Exception as e:
        log(f"Error running via-bench: {e}", Colors.RED)
        return None


def print_formatted_results(results: Dict[str, Any]):
    """Print results in a nice, professional format."""
    print("\n" + "="*80)
    print(f"{Colors.BOLD}{Colors.CYAN}            VIA SOTA PIPELINE EVALUATION REPORT{Colors.END}")
    print("="*80)
    
    # Header Info
    print(f"{Colors.BOLD}Scenario:{Colors.END} {results.get('config_name', 'N/A')}")
    print(f"{Colors.BOLD}Run ID:  {Colors.END} {results.get('run_id', 'N/A')}")
    print("-" * 80)
    
    # High Level Stats
    cols = [
        ("Total Events", f"{results.get('total_events', 0):,}"),
        ("GT Anomalies", f"{results.get('total_ground_truth_anomaly_events', 0):,}"),
        ("Detected", f"{results.get('total_detected_anomalies', 0):,}"),
        ("Throughput", f"{results.get('throughput_eps', 0):.1f} EPS")
    ]
    
    stat_line = " | ".join([f"{Colors.BOLD}{k}:{Colors.END} {v}" for k, v in cols])
    print(stat_line)
    print("-" * 80)
    
    # Tier-1 (Detection) Metrics
    print(f"\n{Colors.BOLD}{Colors.YELLOW}Tier-1 Detection Performance (SOTA Engine){Colors.END}")
    p = results.get('detection_precision', 0) * 100
    r = results.get('detection_recall', 0) * 100
    f1 = results.get('detection_f1', 0)
    
    print(f"  Precision: {get_color_for_metric(p)}{p:6.2f}%{Colors.END} | "
          f"Recall: {get_color_for_metric(r)}{r:6.2f}%{Colors.END} | "
          f"F1: {Colors.BOLD}{f1:.4f}{Colors.END}")
    
    lat_p50 = results.get('detection_latency_p50_micros', 0)
    lat_p95 = results.get('detection_latency_p95_micros', 0)
    print(f"  Latency:   P50: {lat_p50:.1f}µs | P95: {lat_p95:.1f}µs")
    
    # Tier-2 (Incident) Metrics
    print(f"\n{Colors.BOLD}{Colors.YELLOW}Tier-2 Incident Intelligence (Bun/Drizzle/Qdrant){Colors.END}")
    ip = results.get('incident_precision', 0) * 100
    ir = results.get('incident_recall', 0) * 100
    if1 = results.get('incident_f1', 0)
    
    print(f"  Precision: {get_color_for_metric(ip)}{ip:6.2f}%{Colors.END} | "
          f"Recall: {get_color_for_metric(ir)}{ir:6.2f}%{Colors.END} | "
          f"F1: {Colors.BOLD}{if1:.4f}{Colors.END}")
    
    mer = results.get('merge_error_rate', 0) * 100
    ser = results.get('split_error_rate', 0) * 100
    eq = results.get('escalation_quality', 0) * 100
    print(f"  Merge Err: {mer:6.2f}% | Split Err: {ser:6.2f}% | Escalation Qual: {eq:6.2f}%")

    # Anomaly Breakdown
    breakdown = results.get('anomaly_breakdown', [])
    if breakdown:
        print(f"\n{Colors.BOLD}{Colors.YELLOW}Anomaly Breakdown{Colors.END}")
        print(f"  {'Anomaly ID':<30} | {'Scenario':<20} | {'Recall':<10}")
        print(f"  {'-'*30}-+-{'-'*20}-+-{'-'*10}")
        for b in breakdown:
            rec = b.get('recall', 0) * 100
            print(f"  {b.get('anomaly_id', 'N/A')[:30]:<30} | {b.get('scenario', 'N/A')[:20]:<20} | {get_color_for_metric(rec)}{rec:6.1f}%{Colors.END}")

    print("\n" + "="*80)
    
    final_score = (f1 + if1) / 2
    score_color = get_color_for_metric(final_score * 100)
    print(f"{Colors.BOLD}OVERALL PIPELINE HEALTH SCORE: {score_color}{final_score:.4f}{Colors.END}")
    print("="*80 + "\n")


def get_color_for_metric(val: float) -> str:
    if val >= 90: return Colors.GREEN
    if val >= 70: return Colors.CYAN
    if val >= 40: return Colors.YELLOW
    return Colors.RED


def run_evaluation(config: EvalConfig):
    """Run the complete evaluation."""
    log(f"{Colors.BOLD}{Colors.BLUE}VIA SOTA End-to-End Evaluation{Colors.END}", Colors.BLUE)
    log(f"Scenario: {config.scenario} | Seed: {config.seed}", Colors.BLUE)
    print()

    tier2_proc = None

    try:
        # Step 1: Reset database
        if not reset_tier2_tables(config):
            log("Failed to reset environment, continuing anyway...", Colors.YELLOW)

        # Step 2: Start Tier-2
        tier2_proc = start_tier2(config)
        if not tier2_proc:
            log("Failed to start Tier-2", Colors.RED)
            return

        if not wait_for_url(f"{config.tier2_url}/health", timeout=60, name="Tier-2"):
            log("Tier-2 failed to start", Colors.RED)
            return

        # Step 3: Run via-bench pipeline
        results = run_via_bench_pipeline(config)
        
        if results:
            # Step 4: Print results
            print_formatted_results(results)
            return results
        else:
            log("Benchmark failed to produce results", Colors.RED)
            return None

    finally:
        # Cleanup
        log("Cleaning up...", Colors.YELLOW)
        if tier2_proc:
            try:
                # Give Tier-2 a moment to finish any last-second db writes
                time.sleep(2)
                tier2_proc.terminate()
                tier2_proc.wait(timeout=5)
            except:
                tier2_proc.kill()


def main():
    parser = argparse.ArgumentParser(
        description="VIA SOTA End-to-End Evaluation",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )

    parser.add_argument("-s", "--scenario", type=str, default="quick",
                        choices=["quick", "mixed", "mixed_fast", "adversarial", "chaos", "security", "performance", "throughput"],
                        help="Benchmark scenario (default: quick)")
    parser.add_argument("--run-all", action="store_true",
                        help="Run all scenarios sequentially")
    parser.add_argument("-d", "--duration", type=int, default=None,
                        help="Override duration in minutes")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed for deterministic simulation (default: 42)")
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="Verbose output")
    parser.add_argument("--tier2-url", type=str, default="http://127.0.0.1:3000",
                        help="Tier-2 URL (default: http://127.0.0.1:3000)")
    parser.add_argument("-o", "--output", type=str, default=None,
                        help="Output results to JSON file")

    args = parser.parse_args()

    if args.run_all:
        scenarios = ["quick", "mixed_fast", "adversarial", "chaos"]
        all_results = []
        for s in scenarios:
            config = EvalConfig(
                scenario=s,
                duration_minutes=args.duration,
                verbose=args.verbose,
                tier2_url=args.tier2_url,
                seed=args.seed
            )
            print(f"\n{Colors.BOLD}{Colors.MAGENTA}>>> STARTING SCENARIO: {s}{Colors.END}")
            res = run_evaluation(config)
            if res:
                all_results.append(res)
            time.sleep(5) # Cooldown between runs
        
        # Summary
        print(f"\n{Colors.BOLD}{Colors.CYAN}=== ALL-SCENARIO AGGREGATED SUMMARY ==={Colors.END}")
        print(f"{'Scenario':<20} | {'Tier-1 F1':<10} | {'Tier-2 F1':<10} | {'Health'}")
        print("-" * 60)
        for r in all_results:
            name = r.get('config_name', 'N/A')
            t1_f1 = r.get('detection_f1', 0)
            t2_f1 = r.get('incident_f1', 0)
            health = (t1_f1 + t2_f1) / 2
            print(f"{name[:20]:<20} | {t1_f1:10.4f} | {t2_f1:10.4f} | {health:.4f}")
        return

    config = EvalConfig(
        scenario=args.scenario,
        duration_minutes=args.duration,
        verbose=args.verbose,
        tier2_url=args.tier2_url,
        seed=args.seed
    )

    results = run_evaluation(config)

    if results and args.output:
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
