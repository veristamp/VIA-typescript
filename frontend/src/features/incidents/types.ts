export interface Incident {
  id: number;
  incidentId: string;
  status: string;
  entityKey: string;
  firstSeenTs: number;
  lastSeenTs: number;
  severityMax: number;
  scoreMax: number;
  confidence: number;
  evidence: {
    trace_id?: string;
    reason?: string;
    [key: string]: any;
  };
  policyVersion: string;
  createdAt: string;
  updatedAt: string;
}

export interface IncidentsResponse {
  incidents: Incident[];
}
