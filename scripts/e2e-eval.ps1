param(
	[int]$DurationSeconds = 180,
	[int]$Entities = 8,
	[int]$BatchSize = 200
)

$ErrorActionPreference = "Stop"

function Wait-HttpOk {
	param(
		[string]$Url,
		[int]$TimeoutSeconds = 90
	)
	$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
	while ((Get-Date) -lt $deadline) {
		try {
			$resp = Invoke-WebRequest -UseBasicParsing -Uri $Url -TimeoutSec 5
			if ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 300) {
				return $true
			}
		} catch {
			Start-Sleep -Milliseconds 500
		}
	}
	throw "Timed out waiting for $Url"
}

function Stop-ProcIfAlive {
	param([System.Diagnostics.Process]$Proc)
	if ($null -ne $Proc -and -not $Proc.HasExited) {
		try {
			Stop-Process -Id $Proc.Id -Force -ErrorAction Stop
		} catch {}
	}
}

if (!(Test-Path ".runlogs")) {
	New-Item -ItemType Directory -Path ".runlogs" | Out-Null
}

$tier2Out = Resolve-Path ".runlogs"
$tier2Out = Join-Path $tier2Out "tier2.e2e.out.log"
$tier2Err = Resolve-Path ".runlogs"
$tier2Err = Join-Path $tier2Err "tier2.e2e.err.log"
$tier1Out = Resolve-Path ".runlogs"
$tier1Out = Join-Path $tier1Out "tier1.e2e.out.log"
$tier1Err = Resolve-Path ".runlogs"
$tier1Err = Join-Path $tier1Err "tier1.e2e.err.log"

if (Test-Path $tier2Out) { Remove-Item $tier2Out -Force }
if (Test-Path $tier2Err) { Remove-Item $tier2Err -Force }
if (Test-Path $tier1Out) { Remove-Item $tier1Out -Force }
if (Test-Path $tier1Err) { Remove-Item $tier1Err -Force }

# Reset Tier-2 tables for clean run.
bun -e @"
import { Client } from 'pg';
const client = new Client({
  host: 'localhost',
  port: 5432,
  database: 'via_registry',
  user: 'via',
  password: 'via',
});
await client.connect();
await client.query('TRUNCATE TABLE tier2_decisions, tier2_incidents, tier2_dead_letters, evaluation_metrics RESTART IDENTITY');
await client.end();
console.log('db_reset_ok');
"@

# Ensure gatekeeper binary is built.
Push-Location "via-core"
cargo build --release -p via-core --bin gatekeeper | Out-Null
Pop-Location

$tier2Proc = $null
$tier1Proc = $null

try {
	# Start Tier-2 (Bun) on :3000
	$tier2Proc = Start-Process -FilePath "bun" `
		-ArgumentList @("run", "src/main.ts") `
		-WorkingDirectory (Get-Location).Path `
		-RedirectStandardOutput $tier2Out `
		-RedirectStandardError $tier2Err `
		-PassThru

	Wait-HttpOk -Url "http://127.0.0.1:3000/health" -TimeoutSeconds 120 | Out-Null

	# Start Tier-1 gatekeeper on :3001 forwarding to Tier-2
	$tier1Cmd = '$env:TIER2_URL=''http://127.0.0.1:3000''; $env:GATEKEEPER_ADDR=''0.0.0.0:3001''; & ''.\target\release\gatekeeper.exe'''
	$tier1Proc = Start-Process -FilePath "powershell" `
		-ArgumentList @("-NoProfile", "-Command", $tier1Cmd) `
		-WorkingDirectory (Resolve-Path "via-core").Path `
		-RedirectStandardOutput $tier1Out `
		-RedirectStandardError $tier1Err `
		-PassThru

	Wait-HttpOk -Url "http://127.0.0.1:3001/health" -TimeoutSeconds 120 | Out-Null

	$startSec = [int][Math]::Floor((Get-Date -UFormat %s))
	$anomalyRanges = @(
		@{ Start = 40; End = 70 },
		@{ Start = 120; End = 145 }
	)

	$events = New-Object System.Collections.Generic.List[object]
	$groundTruth = @{}

	for ($i = 0; $i -lt $DurationSeconds; $i++) {
		$secTs = $startSec + $i
		$isAnomalySecond = $false
		foreach ($r in $anomalyRanges) {
			if ($i -ge $r.Start -and $i -le $r.End) {
				$isAnomalySecond = $true
				break
			}
		}
		$groundTruth[$secTs] = $isAnomalySecond

		for ($e = 0; $e -lt $Entities; $e++) {
			$uid = "entity_$e"
			$base = 15.0 + ($e * 0.15)
			$noise = (Get-Random -Minimum -10 -Maximum 10) / 100.0
			$val = $base + $noise
			if ($isAnomalySecond) {
				# Strong burst to ensure detector activation.
				$val = 1200.0 + (Get-Random -Minimum 0 -Maximum 200)
			}
			$events.Add(@{
				u = $uid
				v = [double]$val
				t = [UInt64]($secTs * 1000000000)
			}) | Out-Null
		}
	}

	for ($i = 0; $i -lt $events.Count; $i += $BatchSize) {
		$end = [Math]::Min($i + $BatchSize - 1, $events.Count - 1)
		$batch = @()
		for ($j = $i; $j -le $end; $j++) {
			$batch += $events[$j]
		}

		$payload = $batch | ConvertTo-Json -Depth 6
		Invoke-RestMethod -Uri "http://127.0.0.1:3001/ingest/batch" `
			-Method Post `
			-ContentType "application/json" `
			-Body $payload | Out-Null
	}

	# Wait for forwarding + queue flush.
	Start-Sleep -Seconds 12

	$incResp = Invoke-RestMethod -Uri "http://127.0.0.1:3000/analysis/incidents?limit=500" -Method Get
	$incidents = @($incResp.incidents)

	$detectedSeconds = New-Object System.Collections.Generic.HashSet[int]
	foreach ($incident in $incidents) {
		$ts = [int]$incident.lastSeenTs
		if ($ts -ge $startSec -and $ts -lt ($startSec + $DurationSeconds)) {
			$null = $detectedSeconds.Add($ts)
		}
	}

	$tp = 0
	$fp = 0
	$fn = 0
	foreach ($sec in $groundTruth.Keys) {
		$truth = [bool]$groundTruth[$sec]
		$detected = $detectedSeconds.Contains([int]$sec)
		if ($truth -and $detected) { $tp++ }
		elseif (-not $truth -and $detected) { $fp++ }
		elseif ($truth -and -not $detected) { $fn++ }
	}

	$precision = if (($tp + $fp) -eq 0) { 0.0 } else { [double]$tp / ($tp + $fp) }
	$recall = if (($tp + $fn) -eq 0) { 0.0 } else { [double]$tp / ($tp + $fn) }
	$f1 = if (($precision + $recall) -eq 0) { 0.0 } else { (2.0 * $precision * $recall) / ($precision + $recall) }

	$tier1Stats = Invoke-RestMethod -Uri "http://127.0.0.1:3001/stats" -Method Get
	$queueStats = Invoke-RestMethod -Uri "http://127.0.0.1:3000/analysis/pipeline/stats" -Method Get

	$result = [ordered]@{
		window_start = $startSec
		window_end = $startSec + $DurationSeconds - 1
		total_events_sent = $events.Count
		total_incidents = $incidents.Count
		ground_truth_anomaly_seconds = (@($groundTruth.Keys | Where-Object { $groundTruth[$_] }).Count)
		detected_positive_seconds = $detectedSeconds.Count
		tp = $tp
		fp = $fp
		fn = $fn
		precision = [Math]::Round($precision, 4)
		recall = [Math]::Round($recall, 4)
		f1 = [Math]::Round($f1, 4)
		tier1_stats = $tier1Stats
		tier2_queue = $queueStats.queue
	}

	$result | ConvertTo-Json -Depth 8
} finally {
	Stop-ProcIfAlive -Proc $tier1Proc
	Stop-ProcIfAlive -Proc $tier2Proc
}
