import React, { useState } from 'react';
import { Shield, Send, RotateCcw, Play, CheckCircle2, History, AlertCircle } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import client from '../../../api/rpc';

export const PolicyManager: React.FC = () => {
  const queryClient = useQueryClient();
  const [lastAction, setLastAction] = useState<string | null>(null);

  // Queries
  const { data: currentPolicy, isLoading: isCurrentLoading } = useQuery({
    queryKey: ['policy', 'current'],
    queryFn: async () => {
      const res = await client.control.policy.current.$get();
      if (!res.ok) return null;
      return res.json();
    }
  });

  const { data: policies, isLoading: isListLoading } = useQuery({
    queryKey: ['policy', 'list'],
    queryFn: async () => {
      const res = await client.control.policy.$get({ query: { limit: '10' } });
      if (!res.ok) return { policies: [] };
      return res.json();
    }
  });

  // Mutations
  const compileMutation = useMutation({
    mutationFn: async () => {
      const res = await client.control.policy.compile.$post({ json: { limit: 250 } });
      if (!res.ok) throw new Error('Compile failed');
      return res.json();
    },
    onSuccess: (data) => {
      setLastAction(`Compiled new version: ${data.policy_version}`);
      queryClient.invalidateQueries({ queryKey: ['policy', 'list'] });
    }
  });

  const publishMutation = useMutation({
    mutationFn: async (version: string) => {
      const res = await client.control.policy.publish.$post({ json: { policy_version: version } });
      if (!res.ok) throw new Error('Publish failed');
      return res.json();
    },
    onSuccess: (data) => {
      setLastAction(`Published version: ${data.policy_version}`);
      queryClient.invalidateQueries({ queryKey: ['policy'] });
    }
  });

  const rollbackMutation = useMutation({
    mutationFn: async ({ version, reason }: { version: string; reason: string }) => {
      const res = await client.control.policy.rollback.$post({ 
        json: { target_version: version, reason } 
      });
      if (!res.ok) throw new Error('Rollback failed');
      return res.json();
    },
    onSuccess: (data) => {
      setLastAction(`Rolled back to: ${data.policy_version}`);
      queryClient.invalidateQueries({ queryKey: ['policy'] });
    }
  });

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'active': return 'var(--accent-green)';
      case 'rolled_back': return 'var(--accent-red)';
      default: return 'var(--text-muted)';
    }
  };

  return (
    <div className="policy-studio">
      <div className="studio-header">
        <h2><Shield size={20} /> Policy Studio</h2>
        <button 
          className="cyber-button primary" 
          onClick={() => compileMutation.mutate()}
          disabled={compileMutation.isPending}
        >
          {compileMutation.isPending ? <RotateCcw className="spin" size={16} /> : <Play size={16} />}
          COMPILE NEW BASELINE
        </button>
      </div>

      {lastAction && (
        <div className="action-banner">
          <CheckCircle2 size={14} /> {lastAction}
        </div>
      )}

      <div className="studio-layout">
        <div className="current-config">
          <h3>ACTIVE CONFIGURATION</h3>
          {isCurrentLoading ? (
            <p className="loading">Retrieving snapshot...</p>
          ) : currentPolicy?.policy ? (
            <div className="policy-card active">
              <div className="policy-meta">
                <span className="version">{currentPolicy.policy.policyVersion}</span>
                <span className="badge active">ACTIVE</span>
              </div>
              <div className="policy-details">
                <div className="detail-item">
                  <span className="label">Rules:</span>
                  <span className="value">{(currentPolicy.policy.compiledJson as any).rules?.length || 0}</span>
                </div>
                <div className="detail-item">
                  <span className="label">Created:</span>
                  <span className="value">{new Date(currentPolicy.policy.createdAt).toLocaleString()}</span>
                </div>
              </div>
            </div>
          ) : (
            <div className="empty-notice">
              <AlertCircle size={20} />
              <p>No active policy detected in Tier-1.</p>
            </div>
          )}
        </div>

        <div className="policy-history">
          <h3><History size={16} /> ARTIFACT HISTORY</h3>
          {isListLoading ? (
            <p className="loading">Reading registry...</p>
          ) : (
            <div className="history-list">
              {policies?.policies.map((p: any) => (
                <div key={p.policyVersion} className={`history-item ${p.status}`}>
                  <div className="item-info">
                    <span className="version">{p.policyVersion}</span>
                    <span className="status" style={{ color: getStatusColor(p.status) }}>
                      {p.status.toUpperCase()}
                    </span>
                    <span className="timestamp">{new Date(p.createdAt).toLocaleDateString()}</span>
                  </div>
                  <div className="item-actions">
                    {p.status !== 'active' && (
                      <button 
                        className="cyber-button-sm"
                        onClick={() => publishMutation.mutate(p.policyVersion)}
                        disabled={publishMutation.isPending}
                      >
                        <Send size={12} /> ACTIVATE
                      </button>
                    )}
                    {p.status === 'active' && (
                      <button 
                        className="cyber-button-sm danger"
                        onClick={() => {
                          const reason = prompt('Rollback reason:');
                          if (reason) rollbackMutation.mutate({ version: p.rollbackOf || 'previous', reason });
                        }}
                      >
                        <RotateCcw size={12} /> ROLLBACK
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <style>{`
        .policy-studio {
          padding: 1rem;
          background: rgba(0,0,0,0.1);
          border-radius: 8px;
          border: 1px solid var(--border);
        }
        .studio-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 1.5rem;
        }
        .studio-layout {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 2rem;
        }
        .policy-card.active {
          background: var(--bg-card);
          border: 1px solid var(--accent-green);
          padding: 1rem;
          border-radius: 4px;
          box-shadow: 0 0 10px rgba(0, 255, 148, 0.05);
        }
        .history-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }
        .history-item {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 0.75rem;
          background: var(--bg-card);
          border: 1px solid var(--border);
          border-radius: 4px;
        }
        .history-item.active {
          border-left: 3px solid var(--accent-green);
        }
        .item-info {
          display: flex;
          flex-direction: column;
          gap: 0.2rem;
        }
        .item-info .version {
          font-weight: bold;
          font-size: 0.9rem;
        }
        .item-info .timestamp {
          font-size: 0.7rem;
          color: var(--text-muted);
        }
        .item-info .status {
          font-size: 0.6rem;
          font-weight: bold;
          letter-spacing: 1px;
        }
        .cyber-button {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background: var(--bg-hover);
          border: 1px solid var(--border);
          color: var(--text-main);
          border-radius: 4px;
          cursor: pointer;
          font-size: 0.8rem;
          font-weight: bold;
        }
        .cyber-button.primary {
          border-color: var(--accent-blue);
          color: var(--accent-blue);
        }
        .cyber-button:hover {
          background: var(--border);
        }
        .cyber-button-sm {
          display: flex;
          align-items: center;
          gap: 0.3rem;
          padding: 0.3rem 0.6rem;
          background: transparent;
          border: 1px solid var(--border);
          color: var(--text-muted);
          border-radius: 4px;
          cursor: pointer;
          font-size: 0.7rem;
        }
        .cyber-button-sm:hover {
          color: var(--accent-blue);
          border-color: var(--accent-blue);
        }
        .cyber-button-sm.danger:hover {
          color: var(--accent-red);
          border-color: var(--accent-red);
        }
        .action-banner {
          background: rgba(0, 224, 255, 0.1);
          color: var(--accent-blue);
          padding: 0.5rem 1rem;
          border-radius: 4px;
          margin-bottom: 1rem;
          font-size: 0.8rem;
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }
        .spin {
          animation: spin 1s linear infinite;
        }
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        .empty-notice {
          display: flex;
          flex-direction: column;
          align-items: center;
          padding: 2rem;
          color: var(--text-muted);
          gap: 0.5rem;
        }
      `}</style>
    </div>
  );
};
