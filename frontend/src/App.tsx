import { ShieldAlert, Settings, Activity } from "lucide-react";
import { useState } from "react";
import { IncidentGrid } from "./features/incidents/components/IncidentGrid";
import { SignalTicker } from "./features/stream/SignalTicker";
import { PolicyManager } from "./features/policy/components/PolicyManager";
import "./App.css";

// LGTM Deep Link Helpers (assuming Grafana is at :3000)
const GRAFANA_URL = import.meta.env.VITE_GRAFANA_URL || "http://localhost:3000";

function App() {
  const [showPolicy, setShowPolicy] = useState(false);

  return (
    <div className="cyber-app">
      <header className="cyber-header">
        <div className="header-brand">
          <div className="logo">
            <ShieldAlert className="logo-icon" />
            <span>VIA CYBER-SRE</span>
          </div>
          <div className="stats">
            <Activity size={16} /> LIVE
          </div>
        </div>
        
        <div className="header-actions">
          <button 
            className={`cyber-button-icon ${showPolicy ? 'active' : ''}`}
            onClick={() => setShowPolicy(!showPolicy)}
            title="Policy Studio"
          >
            <Settings size={20} />
          </button>
        </div>
      </header>

      <main className="main-layout">
        <div className="content-area">
          {showPolicy ? (
            <div className="policy-view fade-in">
              <PolicyManager />
            </div>
          ) : (
            <div className="incident-view fade-in">
              <IncidentGrid grafanaUrl={GRAFANA_URL} />
            </div>
          )}
        </div>
        
        <aside className="stream-sidebar">
          <div className="sidebar-header">
            <h3>SIGNAL TICKER</h3>
          </div>
          <SignalTicker />
        </aside>
      </main>
    </div>
  );
}

export default App;
