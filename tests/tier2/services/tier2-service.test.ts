import { describe, expect, it } from "bun:test";
import { Tier2QueueService } from "../../../src/services/tier2-queue-service";
import { Tier2Service } from "../../../src/services/tier2-service";

const baseSignal = {
	event_id: "evt-1",
	schema_version: 1,
	entity_hash: "1234",
	timestamp: 1_738_000_000_000_000_000,
	score: 0.92,
	severity: 1,
	primary_detector: 2,
	detectors_fired: 4,
	confidence: 0.88,
	detector_scores: [0.9, 0.7, 0.3],
	attributes: {},
};

describe("Tier2Service", () => {
	it("normalizes Tier-1 severity before persistence and incident seeding", async () => {
		let capturedPayloadSeverity = -1;
		let capturedSeedSeverity = -1;

		const qdrant = {
			ingestToTier2: async (
				events: Array<{ payload: Record<string, unknown> }>,
			) => {
				capturedPayloadSeverity = Number(events[0]?.payload.severity ?? -1);
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
		await service.processAnomalyBatch([{ ...baseSignal, severity: 4 }]);

		expect(capturedPayloadSeverity).toBe(1);
		expect(capturedSeedSeverity).toBe(1);
	});
});

describe("Tier2QueueService", () => {
	it("keeps low Tier-1 severity batches in normal priority after normalization", () => {
		const queue = new Tier2QueueService({
			deriveBatchEventId: () => "evt-low",
		} as never);

		const result = queue.enqueue([{ ...baseSignal, severity: 1 }]);
		expect(result.accepted).toBeTrue();
		expect((queue as any).queue[0].priority).toBe("normal");
	});

	it("marks only critical Tier-1 severity as critical priority", () => {
		const queue = new Tier2QueueService({
			deriveBatchEventId: () => "evt-high",
		} as never);

		const result = queue.enqueue([{ ...baseSignal, severity: 4 }]);
		expect(result.accepted).toBeTrue();
		expect((queue as any).queue[0].priority).toBe("critical");
	});
});
