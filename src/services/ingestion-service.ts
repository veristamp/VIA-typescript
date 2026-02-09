export interface IngestionService {
	ingestLogBatch(logs: Record<string, unknown>[]): Promise<void>;
}
