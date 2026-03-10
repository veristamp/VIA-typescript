# Observability Stack

This stack is intentionally split from the app runtime compose file.

Services:
- Prometheus (metrics scraping)
- Loki (log storage)
- Alloy (container log shipping and OTLP receiver)
- Grafana (dashboards)
- Alertmanager (alert routing)
- Tempo (trace storage and query)

## 1. Create metrics token file

Prometheus uses a bearer token to scrape `/metrics`.

```bash
mkdir -p observability/secrets
openssl rand -hex 32 > observability/secrets/metrics_token
chmod 600 observability/secrets/metrics_token
```

Set the same token in app env (`.env.docker`) as `METRICS_TOKEN`:

```bash
token=$(cat observability/secrets/metrics_token)
sed -i "s|^METRICS_TOKEN=.*|METRICS_TOKEN=${token}|" .env.docker
```

## 2. Start app stack

```bash
docker compose -f docker-compose.build.yml up -d --build
```

## 3. Start observability stack

```bash
docker compose -f docker-compose.observability.yml up -d
```

Or run both together:

```bash
docker compose -f docker-compose.build.yml -f docker-compose.observability.yml up -d --build
```

## 4. Access

- Grafana: http://localhost:3000
- Prometheus: http://localhost:9090
- Loki API: http://localhost:3100
- Alertmanager: http://localhost:9093
- Tempo: http://localhost:3200

## 5. Configure notifications (Slack + webhook)

Alertmanager now renders receiver URLs from environment values at container startup.

Set these in `.env.docker` (or export in shell before `docker compose up`):

```bash
ALERTMANAGER_WEBHOOK_URL="https://your.webhook.receiver/alerts"
ALERTMANAGER_SLACK_WEBHOOK_URL="https://hooks.slack.com/services/xxx/yyy/zzz"
ALERTMANAGER_SLACK_CHANNEL="#alerts"
```

If unset, placeholder URLs are used so the stack still boots safely.

## 6. Configure tracing export from app

Set app env so traces are exported to Tempo:

```bash
OTEL_TRACING_ENABLED=true
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://tempo:4318/v1/traces
```

## 7. Log-to-trace correlation in Grafana

- Loki datasource now extracts `traceId` from structured JSON logs and shows a `View Trace` link.
- Tempo datasource is configured with trace-to-logs query back to Loki.
- If Grafana is already running, restart it to reload provisioned datasource changes:

```bash
docker compose -f docker-compose.observability.yml restart grafana
```

## Notes

- Network is `mono-auth-network` (external), shared with app compose.
- Prometheus scrapes `mono-auth-app:8787/metrics`.
- Prometheus loads core recording rules from `observability/prometheus/recording.yml`, alert rules from `observability/prometheus/rules.yml`, and SLO recording rules from `observability/prometheus/slos.yml`.
- Loki uses explicit retention + compactor settings (14d currently).
- Tempo uses explicit query tuning and block retention flags from compose.
- Alloy ships container stdout/stderr to Loki via Docker socket. Alloy also accepts direct OTLP telemetry from the application (traces, metrics, logs).
- Grafana auto-loads dashboards from `observability/grafana/dashboards/`:
  - `Mono Auth Overview`
  - `Mono Auth Auth Flow`
  - `Mono Auth Database`
  - `Mono Auth Infrastructure`
  - `Mono Auth Investigation`
