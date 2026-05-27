import { describe, expect, it } from "bun:test";
import { Tier2QueueService } from "../../../src/services/tier2-queue-service";
import { Tier2Service } from "../../../src/services/tier2-service";

const baseSignal = {
	event_id: "evt-1",
	schema_version: 1,
	entity_hash: "1234",
	timestamp: 1_738_000_000,
	score: 0.92,
	severity: 0.25,
	primary_detector: 2,
	detectors_fired: 4,
	confidence: 0.88,
	detector_scores: [0.9, 0.7, 0.3],
	attributes: {},
};

describe("Tier2Service", () => {
	it("persists Rust-canonical signal values without TypeScript renormalization", async () => {
		let capturedPayloadSeverity = -1;
		let capturedSeedSeverity = -1;
		let capturedTimestamp = -1;

		const qdrant = {
			ingestToTier2: async (
				events: Array<{ payload: Record<string, unknown> }>,
			) => {
				capturedPayloadSeverity = Number(events[0]?.payload.severity ?? -1);
				capturedTimestamp = Number(events[0]?.payload.timestamp ?? -1);
			},
		};
		const forensic = {
			deriveIncidentCandidates: async (
				_startTs: number,
				_endTs: number,
				seedEvents: Array<{ severity: number }>,
			) => {
				capturedSeedSeverity = seedEvents[0]?.severity ?? -1;
				return [];
			},
		};
		const incidents = {
			applyCandidates: async () => [],
		};

		const service = new Tier2Service(
			qdrant as never,
			forensic as never,
			incidents as never,
		);
		await service.processAnomalyBatch([
			{ ...baseSignal, severity: 0.75, timestamp: 1_738_000_001 },
		]);

		expect(capturedPayloadSeverity).toBe(0.75);
		expect(capturedSeedSeverity).toBe(0.75);
		expect(capturedTimestamp).toBe(1_738_000_001);
	});
});

describe("Tier2QueueService", () => {
	it("keeps low-severity canonical batches in normal priority", () => {
		const queue = new Tier2QueueService({
			deriveBatchEventId: () => "evt-low",
		} as never);

		const result = queue.enqueue([{ ...baseSignal, severity: 0.25 }]);
		expect(result.accepted).toBeTrue();
		expect(
			(queue as unknown as { queue: Array<{ priority: string }> }).queue[0]
				.priority,
		).toBe("normal");
	});

	it("marks critical canonical severity as critical priority", () => {
		const queue = new Tier2QueueService({
			deriveBatchEventId: () => "evt-high",
		} as never);

		const result = queue.enqueue([{ ...baseSignal, severity: 1 }]);
		expect(result.accepted).toBeTrue();
		expect(
			(queue as unknown as { queue: Array<{ priority: string }> }).queue[0]
				.priority,
		).toBe("critical");
	});
});
