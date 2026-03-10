import { Activity, ShieldAlert } from "lucide-react";
import { IncidentGrid } from "./features/incidents/components/IncidentGrid";
import "./App.css";

// LGTM Deep Link Helpers (assuming Grafana is at :3000)
const GRAFANA_URL = import.meta.env.VITE_GRAFANA_URL || "http://localhost:3000";

function App() {
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
        <IncidentGrid grafanaUrl={GRAFANA_URL} />
      </main>
    </div>
  );
}

export default App;
