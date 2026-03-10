import { useQuery } from "@tanstack/react-query";
import { Activity, ExternalLink, ShieldAlert, Terminal } from "lucide-react";
import client from "./api/rpc";
import "./App.css";

// LGTM Deep Link Helpers (assuming Grafana is at :3000)
const GRAFANA_URL = import.meta.env.VITE_GRAFANA_URL || "http://localhost:3000";

const getTempoLink = (traceId: string) => 
  `${GRAFANA_URL}/explore?schemaVersion=1&panes={"v0":{"datasource":"tempo","queries":[{"refId":"A","query":"${traceId}"}],"range":{"from":"now-1h","to":"now"}}}`;

const getLokiLink = (traceId: string) => 
  `${GRAFANA_URL}/explore?schemaVersion=1&panes={"v0":{"datasource":"loki","queries":[{"refId":"A","expr":"{trace_id=\\"${traceId}\\"}"}],"range":{"from":"now-1h","to":"now"}}}`;

function App() {
  const { data, isLoading, error } = useQuery({
    queryKey: ["incidents"],
    queryFn: async () => {
      const res = await client.analysis.incidents.$get();
      if (!res.ok) throw new Error("Failed to fetch incidents");
      return res.json();
    },
    refetchInterval: 5000,
  });

  const getStatusClass = (severity: number) => {
    if (severity >= 0.8) return "status-critical";
    if (severity >= 0.6) return "status-high";
    if (severity >= 0.4) return "status-medium";
    return "status-low";
  };

  return (
    <div className="cyber-app">
      <header className="cyber-header">
        <div className="logo">
          <ShieldAlert className="logo-icon" />
          <span>VIA CYBER-SRE</span>
        </div>
        <div className="stats">
          <Activity size={16} /> Live Stream Active
        </div>
      </header>

      <main>
        <section className="dashboard-section">
          <h2><Terminal size={20} /> Active Incidents</h2>
          {isLoading && <p>Scanning for anomalies...</p>}
          {error && <p className="error">Sensor data error: {error.message}</p>}
          
          <div className="cyber-grid">
            {data?.incidents.map((inc: any) => (
              <div key={inc.incidentId} className="cyber-card">
                <div className="card-header">
                  <span className={`status-indicator ${getStatusClass(inc.severityMax)}`} />
                  <span className="incident-id">{inc.incidentId}</span>
                  <span className="timestamp">
                    {new Date(inc.firstSeenTs * 1000).toLocaleTimeString()}
                  </span>
                </div>
                
                <div className="card-body">
                  <p className="reason">Reason: <strong>{inc.reason}</strong></p>
                  <p className="entity">Entity: <code>{inc.entityKey}</code></p>
                  <p className="score">Confidence: {(inc.confidence * 100).toFixed(1)}%</p>
                </div>

                <div className="card-footer lgtm-bridge">
                  {inc.evidence.trace_id && (
                    <>
                      <a href={getTempoLink(inc.evidence.trace_id)} target="_blank" className="lgtm-link">
                        <ExternalLink size={12} /> TEMPO TRACE
                      </a>
                      <a href={getLokiLink(inc.evidence.trace_id)} target="_blank" className="lgtm-link">
                        <ExternalLink size={12} /> LOKI LOGS
                      </a>
                    </>
                  )}
                  <a href={`${GRAFANA_URL}/d/mono-auth-overview`} target="_blank" className="lgtm-link">
                    <ExternalLink size={12} /> METRICS
                  </a>
                </div>
              </div>
            ))}
          </div>
        </section>
      </main>
    </div>
  );
}

export default App;
