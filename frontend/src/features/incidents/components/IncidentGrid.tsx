import React from 'react';
import { Terminal, RefreshCw, AlertTriangle } from 'lucide-react';
import { useIncidents } from '../hooks/useIncidents';
import { IncidentCard } from './IncidentCard';

interface IncidentGridProps {
  grafanaUrl: string;
}

export const IncidentGrid: React.FC<IncidentGridProps> = ({ grafanaUrl }) => {
  const { data, isLoading, isError, error, isFetching } = useIncidents();

  if (isError) {
    return (
      <div className="error-container">
        <AlertTriangle size={32} />
        <h3>Sensor Data Failure</h3>
        <p>{(error as Error).message}</p>
        <style>{`
          .error-container {
            padding: 2rem;
            border: 1px solid var(--accent-red);
            background: rgba(255, 62, 0, 0.05);
            color: var(--accent-red);
            text-align: center;
            border-radius: 8px;
            margin: 1rem;
          }
        `}</style>
      </div>
    );
  }

  return (
    <section className="dashboard-section">
      <div className="section-header">
        <h2><Terminal size={20} /> Active Incidents</h2>
        <div className={`sync-indicator ${isFetching ? 'syncing' : ''}`}>
          <RefreshCw size={14} />
          <span>{isFetching ? 'FETCHING' : 'SYNCED'}</span>
        </div>
      </div>

      {isLoading ? (
        <div className="loading-state">
          <div className="scanner-line"></div>
          <p>SCANNING NETWORK FOR ANOMALIES...</p>
        </div>
      ) : data?.incidents.length === 0 ? (
        <div className="empty-state">
          <p>NO ACTIVE INCIDENTS DETECTED</p>
        </div>
      ) : (
        <div className="cyber-grid">
          {data?.incidents.map((inc) => (
            <IncidentCard 
              key={inc.incidentId} 
              incident={inc} 
              grafanaUrl={grafanaUrl} 
            />
          ))}
        </div>
      )}

      <style>{`
        .section-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 1.5rem;
          padding: 0 1rem;
        }
        .sync-indicator {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          font-size: 0.7rem;
          color: var(--text-muted);
          background: var(--bg-card);
          padding: 0.2rem 0.6rem;
          border-radius: 10px;
          border: 1px solid var(--border);
        }
        .sync-indicator.syncing {
          color: var(--accent-blue);
          border-color: var(--accent-blue);
        }
        .sync-indicator.syncing svg {
          animation: spin 1s linear infinite;
        }
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        .loading-state, .empty-state {
          height: 200px;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          color: var(--text-muted);
          font-size: 0.8rem;
          letter-spacing: 2px;
          position: relative;
          overflow: hidden;
          background: rgba(0,0,0,0.2);
          border-radius: 8px;
          margin: 1rem;
        }
        .scanner-line {
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 2px;
          background: var(--accent-blue);
          box-shadow: 0 0 15px var(--accent-blue);
          animation: scan 2s linear infinite;
          opacity: 0.5;
        }
        @keyframes scan {
          0% { top: 0; }
          100% { top: 100%; }
        }
      `}</style>
    </section>
  );
};
