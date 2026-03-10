import React from 'react';
import { ExternalLink, Clock, Database, Fingerprint } from 'lucide-react';
import { Incident } from '../types';

interface IncidentCardProps {
  incident: Incident;
  grafanaUrl: string;
}

const getTempoLink = (grafanaUrl: string, traceId: string) => 
  `${grafanaUrl}/explore?schemaVersion=1&panes={"v0":{"datasource":"tempo","queries":[{"refId":"A","query":"${traceId}"}],"range":{"from":"now-1h","to":"now"}}}`;

const getLokiLink = (grafanaUrl: string, traceId: string) => 
  `${grafanaUrl}/explore?schemaVersion=1&panes={"v0":{"datasource":"loki","queries":[{"refId":"A","expr":"{trace_id=\\"${traceId}\\"}"}],"range":{"from":"now-1h","to":"now"}}}`;

const getStatusClass = (severity: number) => {
  if (severity >= 80) return "status-critical"; // Severity in DB is integer 0-100
  if (severity >= 60) return "status-high";
  if (severity >= 40) return "status-medium";
  return "status-low";
};

export const IncidentCard: React.FC<IncidentCardProps> = ({ incident, grafanaUrl }) => {
  return (
    <div className="cyber-card">
      <div className="card-header">
        <div className="status-group">
          <span className={`status-indicator ${getStatusClass(incident.severityMax)}`} />
          <span className="incident-id">{incident.incidentId}</span>
        </div>
        <span className="timestamp">
          <Clock size={12} style={{ marginRight: '4px' }} />
          {new Date(incident.lastSeenTs * 1000).toLocaleTimeString()}
        </span>
      </div>
      
      <div className="card-body">
        <div className="data-row">
          <Fingerprint size={14} className="icon-muted" />
          <code>{incident.entityKey}</code>
        </div>
        <div className="data-row">
          <Database size={14} className="icon-muted" />
          <span className="reason-text">{incident.evidence.reason || 'Unknown anomaly'}</span>
        </div>
        
        <div className="metrics-row">
          <div className="metric">
            <span className="label">SEVERITY</span>
            <span className="value">{incident.severityMax}%</span>
          </div>
          <div className="metric">
            <span className="label">CONFIDENCE</span>
            <span className="value">{incident.confidence}%</span>
          </div>
        </div>
      </div>

      <div className="card-footer lgtm-bridge">
        {incident.evidence.trace_id && (
          <>
            <a href={getTempoLink(grafanaUrl, incident.evidence.trace_id)} target="_blank" rel="noreferrer" className="lgtm-link">
              <ExternalLink size={12} /> TEMPO
            </a>
            <a href={getLokiLink(grafanaUrl, incident.evidence.trace_id)} target="_blank" rel="noreferrer" className="lgtm-link">
              <ExternalLink size={12} /> LOKI
            </a>
          </>
        )}
        <a href={`${grafanaUrl}/d/mono-auth-overview`} target="_blank" rel="noreferrer" className="lgtm-link">
          <ExternalLink size={12} /> METRICS
        </a>
      </div>

      <style>{`
        .data-row {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          margin: 0.5rem 0;
          font-size: 0.85rem;
        }
        .icon-muted {
          color: var(--text-muted);
        }
        .reason-text {
          color: var(--text-main);
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .metrics-row {
          display: flex;
          gap: 1rem;
          margin-top: 1rem;
          padding: 0.5rem;
          background: rgba(0,0,0,0.2);
          border-radius: 4px;
        }
        .metric {
          display: flex;
          flex-direction: column;
        }
        .metric .label {
          font-size: 0.6rem;
          color: var(--text-muted);
        }
        .metric .value {
          font-size: 0.9rem;
          font-weight: bold;
          color: var(--accent-blue);
        }
        .status-group {
          display: flex;
          align-items: center;
        }
      `}</style>
    </div>
  );
};
