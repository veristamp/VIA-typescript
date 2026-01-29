import { Hono } from "hono";
import { analysisRoutes } from "./api/routes/analysis";
import { controlRoutes } from "./api/routes/control";
import { evaluationRoutes } from "./api/routes/evaluation";
import { healthRoutes } from "./api/routes/health";
import { ingestRoutes } from "./api/routes/ingest";
import { schemaRoutes } from "./api/routes/schema";
import { simulationRoutes } from "./api/routes/simulation";
import { streamRoutes } from "./api/routes/stream";
import { settings } from "./config/settings";
import { QueueWorker } from "./queue/worker";
import {
	ControlService,
	ForensicAnalysisService,
	IngestionService,
	PromotionService,
	QdrantService,
	SchemaService,
	Tier1Engine,
} from "./services";
import { EvaluationService } from "./services/evaluation-service";
import { Simulator } from "./simulation/simulator";

// Initialize services
const qdrantService = new QdrantService();
const tier1Engine = new Tier1Engine(settings);
const worker = new QueueWorker(tier1Engine, {
	batchSize: settings.queue.batchSize,
	flushInterval: settings.queue.flushInterval,
});
const ingestionService = new IngestionService(
	qdrantService,
	tier1Engine,
	worker,
);
const schemaService = new SchemaService();
const controlService = new ControlService();
const forensicAnalysisService = new ForensicAnalysisService(qdrantService);
const promotionService = new PromotionService(qdrantService);
const evaluationService = new EvaluationService();
const simulator = new Simulator(ingestionService, evaluationService);

// Setup Hono app
const app = new Hono();

// Middleware to inject services
app.use("*", async (c, next) => {
	c.set("ingestionService", ingestionService);
	c.set("tier1Engine", tier1Engine);
	c.set("schemaService", schemaService);
	c.set("controlService", controlService);
	c.set("forensicAnalysisService", forensicAnalysisService);
	c.set("promotionService", promotionService);
	c.set("simulator", simulator);
	c.set("evaluationService", evaluationService);
	await next();
});

// Register routes
app.route("/", healthRoutes);
app.route("/ingest", ingestRoutes);
app.route("/control", controlRoutes);
app.route("/schema", schemaRoutes);
app.route("/analysis", analysisRoutes);
app.route("/stream", streamRoutes);
app.route("/simulation", simulationRoutes);
app.route("/evaluation", evaluationRoutes);

// Initialize application
async function initialize() {
	console.log("Initializing VIA v2 Backend...");

	// Setup Qdrant collections
	await qdrantService.setupCollections();
	console.log("Qdrant collections initialized");

	// Start queue worker
	worker.start();
	console.log("Queue worker started");

	console.log("VIA v2 Backend initialized successfully");
}

// Start server
async function startServer() {
	await initialize();

	const port = settings.server.port;
	const host = settings.server.host;

	console.log(`Starting server on ${host}:${port}`);

	Bun.serve({
		fetch: app.fetch,
		port,
		hostname: host,
	});

	console.log(`Server running on http://${host}:${port}`);
}

// Handle graceful shutdown
process.on("SIGINT", () => {
	console.log("Shutting down gracefully...");
	worker.stop();
	simulator.stopScenario();
	process.exit(0);
});

process.on("SIGTERM", () => {
	console.log("Shutting down gracefully...");
	worker.stop();
	simulator.stopScenario();
	process.exit(0);
});

// Start the server
startServer().catch((error) => {
	console.error("Failed to start server:", error);
	process.exit(1);
});

export {
	app,
	qdrantService,
	ingestionService,
	tier1Engine,
	worker,
	schemaService,
	controlService,
	forensicAnalysisService,
	promotionService,
	evaluationService,
	simulator,
};
