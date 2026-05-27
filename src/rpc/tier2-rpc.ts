import * as http2 from "node:http2";
import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ConnectRouter } from "@connectrpc/connect";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { settings } from "../config/settings";
import {
	SubmitAnomalyBatchResponseSchema,
	type Tier1Signal,
	Tier2Service,
} from "../gen/via/tier2/v1/tier2_pb";
import { Tier1V1AnomalyBatchSchema } from "../modules/tier2/contracts/tier1-signal";
import type { Tier2QueueService } from "../services/tier2-queue-service";
import { logger } from "../utils/logger";

function toUnixSeconds(timestamp: bigint): number {
	const value = Number(timestamp);
	if (!Number.isSafeInteger(value)) {
		throw new ConnectError(
			"timestamp is outside safe integer range",
			Code.InvalidArgument,
		);
	}
	return value;
}

function signalToContract(signal: Tier1Signal) {
	return {
		event_id: signal.eventId,
		schema_version: signal.schemaVersion,
		entity_hash: signal.entityHash,
		timestamp: toUnixSeconds(signal.timestamp),
		score: signal.score,
		severity: signal.severity,
		primary_detector: signal.primaryDetector,
		detectors_fired: signal.detectorsFired,
		confidence: signal.confidence,
		detector_scores: signal.detectorScores,
		attributes: signal.attributes,
	};
}

export function createTier2RpcRoutes(queue: Tier2QueueService) {
	return (router: ConnectRouter) => {
		router.service(Tier2Service, {
			submitAnomalyBatch(request) {
				const parsed = Tier1V1AnomalyBatchSchema.safeParse({
					signals: request.signals.map(signalToContract),
				});
				if (!parsed.success) {
					throw new ConnectError(parsed.error.message, Code.InvalidArgument);
				}

				const result = queue.enqueue(parsed.data.signals);
				return create(SubmitAnomalyBatchResponseSchema, {
					status: result.accepted ? "accepted" : "rejected",
					eventId: result.eventId,
					reason: result.reason ?? "",
				});
			},
		});
	};
}

export function startTier2RpcServer(queue: Tier2QueueService) {
	const server = http2.createServer(
		connectNodeAdapter({
			routes: createTier2RpcRoutes(queue),
			grpc: true,
			grpcWeb: false,
			connect: true,
		}),
	);

	server.listen(settings.rpc.port, settings.rpc.host);
	logger.info("Tier-2 RPC server running", {
		host: settings.rpc.host,
		port: settings.rpc.port,
		protocols: ["grpc", "connect"],
	});

	return server;
}
