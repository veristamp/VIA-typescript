#!/usr/bin/env python3
"""
VIA End-to-End Evaluation Script

Evaluates the tier1+tier2 pipeline by:
1. Starting Tier-2 (Bun) server
2. Starting Tier-1 (gatekeeper) forwarding to Tier-2
3. Generating synthetic events with ground truth anomalies
4. Measuring precision, recall, and F1 score

Usage:
    python scripts/e2e-eval.py                    # Run with defaults
    python scripts/e2e-eval.py --duration 120     # 2 minute test
    python scripts/e2e-eval.py --entities 16      # 16 entities
    python scripts/e2e-eval.py --batch-size 100   # Batch size 100
    python scripts/e2e-eval.py --tier1-only        # Test tier1 only (no forwarding)
    python scripts/e2e-eval.py --verbose          # Verbose output
"""

import argparse
import json
import os
import random
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

import requests


@dataclass
class EvalConfig:
    duration_seconds: int = 180
    entities: int = 8
    batch_size: int = 200
    tier1_only: bool = False
    verbose: bool = False
    tier2_url: str = "http://127.0.0.1:3000"
    tier1_url: str = "http://127.0.0.1:3001"
    db_host: str = "localhost"
    db_port: int = 5432
    db_name: str = "via_registry"
    db_user: str = "via"
    db_password: str = "via"


class Colors:
    GREEN = "\033[92m"
    RED = "\033[91m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    BOLD = "\033[1m"
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
        time.sleep(0.5)
    log(f"Timeout waiting for {name} at {url}", Colors.RED)
    return False


def reset_tier2_tables(config: EvalConfig) -> bool:
    """Reset Tier-2 database tables and Qdrant."""
    log("Resetting Tier-2 tables...", Colors.YELLOW, config.verbose)

    # Clear Qdrant collection
    try:
        import requests as req
        resp = req.get(f"{config.tier2_url}/health", timeout=5)
        if resp.status_code == 200:
            # Get collection name and clear it
            coll_resp = req.get("http://localhost:6333/collections")
            if coll_resp.status_code == 200:
                collections = coll_resp.json().get("result", {}).get("collections", [])
                for coll in collections:
                    name = coll.get("name", "")
                    if name.startswith("via_forensic"):
                        req.post(
                            f"http://localhost:6333/collections/{name}/points/delete",
                            json={"filter": {}},
                            timeout=10
                        )
            log("Qdrant cleared", Colors.GREEN, config.verbose)
    except Exception as e:
        log(f"Qdrant cleanup warning: {e}", Colors.YELLOW, config.verbose)

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
            log("Tier-2 tables reset", Colors.GREEN, config.verbose)
            return True
        else:
            log(f"Failed to reset tables: {result.stderr}", Colors.RED)
            return False
    except FileNotFoundError:
        log("psql not found - skipping database reset (may not be needed)", Colors.YELLOW, config.verbose)
        return True
    except Exception as e:
        log(f"Database reset error: {e}", Colors.YELLOW, config.verbose)
        return True


def build_gatekeeper(config: EvalConfig) -> bool:
    """Build the gatekeeper binary."""
    log("Building gatekeeper...", Colors.YELLOW, config.verbose)

    via_core_path = Path(__file__).parent.parent / "via-core"
    if not via_core_path.exists():
        log(f"via-core not found at {via_core_path}", Colors.RED)
        return False

    try:
        result = subprocess.run(
            ["cargo", "build", "--release", "-p", "via-core", "--bin", "gatekeeper"],
            cwd=via_core_path,
            capture_output=True,
            text=True,
            timeout=300
        )
        if result.returncode == 0:
            log("Gatekeeper built", Colors.GREEN, config.verbose)
            return True
        else:
            log(f"Build failed: {result.stderr}", Colors.RED)
            return False
    except Exception as e:
        log(f"Build error: {e}", Colors.RED)
        return False


def start_tier2(config: EvalConfig) -> Optional[subprocess.Popen]:
    """Start Tier-2 (Bun) server."""
    log("Starting Tier-2 (Bun)...", Colors.YELLOW, config.verbose)

    try:
        proc = subprocess.Popen(
            ["bun", "run", "src/main.ts"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=Path(__file__).parent.parent
        )
        return proc
    except Exception as e:
        log(f"Failed to start Tier-2: {e}", Colors.RED)
        return None


def start_tier1(config: EvalConfig) -> Optional[subprocess.Popen]:
    """Start Tier-1 (gatekeeper) server."""
    log("Starting Tier-1 (gatekeeper)...", Colors.YELLOW, config.verbose)

    via_core_path = Path(__file__).parent.parent / "via-core"
    gatekeeper_bin = via_core_path / "target" / "release" / "gatekeeper"

    if not gatekeeper_bin.exists():
        log(f"Gatekeeper binary not found at {gatekeeper_bin}", Colors.RED)
        return None

    env = os.environ.copy()
    if not config.tier1_only:
        env["TIER2_URL"] = config.tier2_url
    env["GATEKEEPER_ADDR"] = "0.0.0.0:3001"

    try:
        proc = subprocess.Popen(
            [str(gatekeeper_bin)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
            cwd=via_core_path
        )
        return proc
    except Exception as e:
        log(f"Failed to start Tier-1: {e}", Colors.RED)
        return None


def generate_events(config: EvalConfig):
    """Generate synthetic events with ground truth anomalies.
    
    Note: The tier1 detectors are tuned for benchmark-style log data.
    Simple counter events may not trigger high detection scores.
    This generates values that attempt to trigger ChangePoint/Burst detectors.
    """
    log(f"Generating {config.duration_seconds * config.entities} events...", Colors.YELLOW, config.verbose)

    start_sec = int(time.time())
    warmup_seconds = 20
    anomaly_ranges = [
        (30, 50),
    ]

    events = []
    ground_truth = {}
    
    prev_values = {e: 15.0 + (e * 0.15) for e in range(config.entities)}

    for i in range(config.duration_seconds):
        sec_ts = start_sec + i
        is_anomaly = any(r[0] <= i <= r[1] for r in anomaly_ranges)
        is_warmup = i < warmup_seconds
        ground_truth[sec_ts] = is_anomaly

        for e in range(config.entities):
            uid = f"entity_{e}"
            base = 15.0 + (e * 0.15)
            prev_val = prev_values[e]
            
            if is_anomaly and not is_warmup:
                if i % 3 == 0:
                    val = prev_val * random.uniform(3, 8)
                else:
                    val = prev_val + random.uniform(50, 200)
            else:
                drift = random.uniform(-2, 2)
                val = base + drift
            
            val = max(0.1, val)
            prev_values[e] = val

            events.append({
                "u": uid,
                "v": val,
                "t": sec_ts * 1_000_000_000  # nanoseconds
            })

    return events, ground_truth, start_sec


def send_events(config: EvalConfig, events: list) -> bool:
    """Send events to Tier-1."""
    tier1_url = config.tier1_url if not config.tier1_only else config.tier2_url
    log(f"Sending {len(events)} events to {tier1_url}...", Colors.YELLOW, config.verbose)

    total_sent = 0
    for i in range(0, len(events), config.batch_size):
        batch = events[i:i + config.batch_size]
        try:
            resp = requests.post(
                f"{tier1_url}/ingest/batch",
                json=batch,
                timeout=30
            )
            if resp.status_code == 200 or resp.status_code == 202:
                total_sent += len(batch)
            else:
                log(f"Failed to send batch: {resp.status_code}", Colors.RED)
        except Exception as e:
            log(f"Send error: {e}", Colors.RED)

    log(f"Sent {total_sent}/{len(events)} events", Colors.GREEN, config.verbose)
    return total_sent > 0


def query_incidents(config: EvalConfig, start_sec: int, duration: int) -> list:
    """Query incidents from Tier-2."""
    log("Querying incidents from Tier-2...", Colors.YELLOW, config.verbose)

    try:
        resp = requests.get(f"{config.tier2_url}/analysis/incidents?limit=500", timeout=30)
        if resp.status_code == 200:
            data = resp.json()
            incidents = data.get("incidents", [])
            log(f"Retrieved {len(incidents)} incidents", Colors.GREEN, config.verbose)
            return incidents
        else:
            log(f"Failed to query incidents: {resp.status_code}", Colors.RED)
            return []
    except Exception as e:
        log(f"Query error: {e}", Colors.RED)
        return []


def calculate_metrics(ground_truth: dict, detected_seconds: set, start_sec: int, duration: int):
    """Calculate precision, recall, and F1."""
    tp = fp = fn = 0

    for sec in range(start_sec, start_sec + duration):
        truth = ground_truth.get(sec, False)
        detected = sec in detected_seconds

        if truth and detected:
            tp += 1
        elif not truth and detected:
            fp += 1
        elif truth and not detected:
            fn += 1

    precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
    recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
    f1 = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0

    return {
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "precision": round(precision, 4),
        "recall": round(recall, 4),
        "f1": round(f1, 4)
    }


def get_stats(config: EvalConfig) -> dict:
    """Get stats from Tier-1 and Tier-2."""
    stats = {}

    try:
        resp = requests.get(f"{config.tier1_url}/stats", timeout=10)
        if resp.status_code == 200:
            stats["tier1"] = resp.json()
    except:
        pass

    try:
        resp = requests.get(f"{config.tier2_url}/analysis/pipeline/stats", timeout=10)
        if resp.status_code == 200:
            stats["tier2"] = resp.json()
    except:
        pass

    return stats


def run_evaluation(config: EvalConfig):
    """Run the complete evaluation."""
    log(f"{Colors.BOLD}VIA End-to-End Evaluation{Colors.END}", Colors.BLUE)
    log(f"Duration: {config.duration_seconds}s | Entities: {config.entities} | Batch: {config.batch_size}", Colors.BLUE)
    if config.tier1_only:
        log(f"Mode: Tier-1 Only (no forwarding)", Colors.YELLOW)
    else:
        log(f"Mode: Tier-1 + Tier-2 Pipeline", Colors.YELLOW)
    print()

    tier1_proc = None
    tier2_proc = None

    try:
        # Step 1: Reset database
        if not reset_tier2_tables(config):
            log("Failed to reset tables, continuing anyway...", Colors.YELLOW)

        # Step 2: Build gatekeeper
        if not config.tier1_only:
            if not build_gatekeeper(config):
                log("Failed to build gatekeeper", Colors.RED)
                return

        # Step 3: Start Tier-2
        tier2_proc = start_tier2(config)
        if not tier2_proc:
            log("Failed to start Tier-2", Colors.RED)
            return

        if not wait_for_url(f"{config.tier2_url}/health", timeout=120, name="Tier-2"):
            log("Tier-2 failed to start", Colors.RED)
            return

        # Step 4: Start Tier-1
        if not config.tier1_only:
            tier1_proc = start_tier1(config)
            if not tier1_proc:
                log("Failed to start Tier-1", Colors.RED)
                return

            if not wait_for_url(f"{config.tier1_url}/health", timeout=120, name="Tier-1"):
                log("Tier-1 failed to start", Colors.RED)
                return

        # Step 5: Generate events
        events, ground_truth, start_sec = generate_events(config)

        # Count anomaly seconds in ground truth
        anomaly_seconds = sum(1 for v in ground_truth.values() if v)
        log(f"Ground truth: {anomaly_seconds} anomaly seconds out of {config.duration_seconds}", Colors.BLUE, config.verbose)

        # Step 6: Send events
        if not send_events(config, events):
            log("Failed to send events", Colors.RED)
            return

        # Step 7: Wait for forwarding and processing
        log("Waiting for forwarding + processing...", Colors.YELLOW)
        time.sleep(20)

        # Step 8: Query results
        incidents = []
        if config.tier1_only:
            # For tier1-only, we can't query incidents from tier2
            log("Tier-1 only mode - cannot query incidents", Colors.YELLOW)
            metrics = {"tp": 0, "fp": 0, "fn": 0, "precision": 0, "recall": 0, "f1": 0}
            detected_seconds = set()
        else:
            incidents = query_incidents(config, start_sec, config.duration_seconds)

            # Extract detected seconds
            detected_seconds = set()
            for incident in incidents:
                ts = incident.get("lastSeenTs")
                if ts and start_sec <= ts < start_sec + config.duration_seconds:
                    detected_seconds.add(ts)

            # Calculate metrics
            metrics = calculate_metrics(ground_truth, detected_seconds, start_sec, config.duration_seconds)

        # Step 9: Get stats
        stats = get_stats(config)

        # Step 10: Print results
        print()
        log(f"{Colors.BOLD}=== EVALUATION RESULTS ==={Colors.END}", Colors.BLUE)
        print()
        print(f"  Window: {start_sec} - {start_sec + config.duration_seconds - 1}")
        print(f"  Total events sent: {len(events)}")
        print(f"  Total incidents: {len(incidents) if not config.tier1_only else 'N/A'}")
        print(f"  Ground truth anomaly seconds: {anomaly_seconds}")
        print(f"  Detected positive seconds: {len(detected_seconds) if not config.tier1_only else 'N/A'}")
        print()
        print(f"  True Positives:  {metrics['tp']}")
        print(f"  False Positives: {metrics['fp']}")
        print(f"  False Negatives: {metrics['fn']}")
        print()
        print(f"  Precision: {metrics['precision']:.2%}")
        print(f"  Recall:    {metrics['recall']:.2%}")
        print(f"  F1 Score:  {metrics['f1']:.4f}")
        print()

        if stats:
            if "tier1" in stats:
                print(f"  Tier-1 Stats:")
                t1 = stats["tier1"]
                print(f"    Version: {t1.get('version', 'N/A')}")
                print(f"    Forwarding: {t1.get('tier2_forwarding_enabled', False)}")
                if t1.get('tier2_forwarding_enabled'):
                    print(f"    Forwarded: {t1.get('tier2_forwarded_sent', 0)}")
                    print(f"    Failed: {t1.get('tier2_forwarded_failed', 0)}")
            if "tier2" in stats and "queue" in stats["tier2"]:
                print(f"  Tier-2 Queue:")
                q = stats["tier2"]["queue"]
                print(f"    Queued: {q.get('queued', 0)}")
                print(f"    Processed: {q.get('processed', 0)}")
                print(f"    In Flight: {q.get('inFlight', 0)}")

        print()
        log("Evaluation complete!", Colors.GREEN)

        return {
            "window_start": start_sec,
            "window_end": start_sec + config.duration_seconds - 1,
            "total_events_sent": len(events),
            "total_incidents": len(incidents) if not config.tier1_only else None,
            "ground_truth_anomaly_seconds": anomaly_seconds,
            "detected_positive_seconds": len(detected_seconds) if not config.tier1_only else None,
            "metrics": metrics,
            "stats": stats
        }

    finally:
        # Cleanup
        log("Cleaning up...", Colors.YELLOW, config.verbose)

        if tier1_proc:
            try:
                tier1_proc.terminate()
                tier1_proc.wait(timeout=5)
            except:
                tier1_proc.kill()

        if tier2_proc:
            try:
                tier2_proc.terminate()
                tier2_proc.wait(timeout=5)
            except:
                tier2_proc.kill()


def main():
    parser = argparse.ArgumentParser(
        description="VIA End-to-End Evaluation",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )

    parser.add_argument("-d", "--duration", type=int, default=180,
                        help="Test duration in seconds (default: 180)")
    parser.add_argument("-e", "--entities", type=int, default=8,
                        help="Number of entities (default: 8)")
    parser.add_argument("-b", "--batch-size", type=int, default=200,
                        help="Batch size for event sending (default: 200)")
    parser.add_argument("--tier1-only", action="store_true",
                        help="Run tier1 only (no tier2 forwarding)")
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="Verbose output")
    parser.add_argument("--tier2-url", type=str, default="http://127.0.0.1:3000",
                        help="Tier-2 URL (default: http://127.0.0.1:3000)")
    parser.add_argument("--tier1-url", type=str, default="http://127.0.0.1:3001",
                        help="Tier-1 URL (default: http://127.0.0.1:3001)")
    parser.add_argument("--db-host", type=str, default="localhost",
                        help="Database host (default: localhost)")
    parser.add_argument("--db-port", type=int, default=5432,
                        help="Database port (default: 5432)")
    parser.add_argument("--db-name", type=str, default="via_registry",
                        help="Database name (default: via_registry)")
    parser.add_argument("--db-user", type=str, default="via",
                        help="Database user (default: via)")
    parser.add_argument("--db-password", type=str, default="via",
                        help="Database password (default: via)")
    parser.add_argument("-o", "--output", type=str, default=None,
                        help="Output results to JSON file")

    args = parser.parse_args()

    config = EvalConfig(
        duration_seconds=args.duration,
        entities=args.entities,
        batch_size=args.batch_size,
        tier1_only=args.tier1_only,
        verbose=args.verbose,
        tier2_url=args.tier2_url,
        tier1_url=args.tier1_url,
        db_host=args.db_host,
        db_port=args.db_port,
        db_name=args.db_name,
        db_user=args.db_user,
        db_password=args.db_password,
    )

    result = run_evaluation(config)

    if result and args.output:
        with open(args.output, "w") as f:
            json.dump(result, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
