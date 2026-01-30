import { Hono } from "hono";
import { analysisRoutes } from "./api/routes/analysis";
import { controlRoutes } from "./api/routes/control";
import { evaluationRoutes } from "./api/routes/evaluation";
import { healthRoutes } from "./api/routes/health";
import { schemaRoutes } from "./api/routes/schema";
import { simulationRoutes } from "./api/routes/simulation";
import { streamRoutes } from "./api/routes/stream";
import { settings } from "./config/settings";
import {
	ControlService,
	ForensicAnalysisService,
	QdrantService,
	SchemaService,
} from "./services";
import { Tier2Service } from "./services/tier2-service";
import { EvaluationService } from "./services/evaluation-service";

// Initialize services
const qdrantService = new QdrantService();
const schemaService = new SchemaService();
const controlService = new ControlService();
const forensicAnalysisService = new ForensicAnalysisService(qdrantService);
const evaluationService = new EvaluationService();
const tier2Service = new Tier2Service(qdrantService, forensicAnalysisService);

// Setup Hono app
const app = new Hono();

// Middleware to inject services
app.use("*", async (c, next) => {
	c.set("tier2Service", tier2Service);
	c.set("schemaService", schemaService);
	c.set("controlService", controlService);
	c.set("forensicAnalysisService", forensicAnalysisService);
	c.set("evaluationService", evaluationService);
	await next();
});

// Register routes
app.route("/", healthRoutes);
app.route("/control", controlRoutes);
app.route("/schema", schemaRoutes);
app.route("/analysis", analysisRoutes);
app.route("/stream", streamRoutes); // Now /tier2/anomalies
app.route("/simulation", simulationRoutes);
app.route("/evaluation", evaluationRoutes);

// Initialize application
async function initialize() {
	console.log("Initializing VIA v2 Backend (Tier-2 Focus)...");

	// Setup Qdrant collections
	await qdrantService.setupCollections();
	console.log("Qdrant collections initialized");

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
	process.exit(0);
});

process.on("SIGTERM", () => {
	console.log("Shutting down gracefully...");
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
	schemaService,
	controlService,
	forensicAnalysisService,
	evaluationService,
	tier2Service,
};
