import { useEffect, useState, useRef } from 'react';
import { Activity, Zap } from 'lucide-react';

interface Signal {
  entity_hash: string;
  score: number;
  severity: number;
  primary_detector: number;
  timestamp: number;
}

export const SignalTicker = () => {
  const [signals, setSignals] = useState<Signal[]>([]);
  const [connected, setConnected] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const url = `${import.meta.env.VITE_API_URL || 'http://localhost:8787'}/signals`;
    const eventSource = new EventSource(url);

    eventSource.addEventListener('info', (e) => {
      if (e.data === 'connected') setConnected(true);
    });

    eventSource.addEventListener('signals', (e) => {
      const newSignals: Signal[] = JSON.parse(e.data);
      setSignals(prev => [...newSignals, ...prev].slice(0, 50));
    });

    eventSource.onerror = () => {
      setConnected(false);
    };

    return () => {
      eventSource.close();
    };
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  }, [signals]);

  return (
    <div className="signal-ticker cyber-card">
      <div className="ticker-header">
        <div className="flex items-center gap-2">
          <Activity size={16} className={connected ? 'text-accent-green' : 'text-accent-red'} />
          <span className="text-sm font-bold">LIVE SIGNAL STREAM</span>
        </div>
        {connected && <Zap size={12} className="text-accent-blue animate-pulse" />}
      </div>
      
      <div className="ticker-viewport" ref={scrollRef}>
        {signals.length === 0 ? (
          <div className="p-4 text-center text-xs text-text-muted">
            Waiting for Tier-1 telemetry...
          </div>
        ) : (
          signals.map((sig, i) => (
            <div key={`${sig.entity_hash}-${sig.timestamp}-${i}`} className="ticker-item">
              <span className="ticker-ts">{new Date(sig.timestamp / 1000000).toLocaleTimeString()}</span>
              <span className="ticker-entity">{sig.entity_hash.slice(0, 8)}</span>
              <span className={`ticker-score ${sig.score > 0.5 ? 'text-accent-orange' : 'text-text-muted'}`}>
                {(sig.score * 100).toFixed(0)}%
              </span>
            </div>
          ))
        )}
      </div>

      <style>{`
        .signal-ticker {
          height: 300px;
          display: flex;
          flex-direction: column;
          overflow: hidden;
          padding: 0;
        }
        .ticker-header {
          padding: 0.75rem 1rem;
          border-bottom: 1px solid var(--border);
          display: flex;
          justify-content: space-between;
          align-items: center;
          background: rgba(0,0,0,0.2);
        }
        .ticker-viewport {
          flex: 1;
          overflow-y: auto;
          font-size: 0.75rem;
          padding: 0.5rem;
        }
        .ticker-item {
          display: flex;
          gap: 1rem;
          padding: 0.25rem 0.5rem;
          border-bottom: 1px solid rgba(255,255,255,0.05);
          animation: slideIn 0.3s ease-out;
        }
        .ticker-ts { color: var(--text-muted); width: 70px; }
        .ticker-entity { color: var(--accent-blue); flex: 1; font-family: var(--font-mono); }
        .ticker-score { width: 40px; text-align: right; }
        
        @keyframes slideIn {
          from { opacity: 0; transform: translateY(-10px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
};
