import { Hono } from "hono";
import { analysisRoutes } from "./api/routes/analysis";
import { controlRoutes } from "./api/routes/control";
import { evaluationRoutes } from "./api/routes/evaluation";
import { healthRoutes } from "./api/routes/health";
import { schemaRoutes } from "./api/routes/schema";
import { streamRoutes } from "./api/routes/stream";
import { settings } from "./config/settings";
import { initializeRegistry } from "./db/registry";
import {
	ControlService,
	ForensicAnalysisService,
	IncidentService,
	QdrantService,
	SchemaService,
	Tier2QueueService,
} from "./services";
import { EvaluationService } from "./services/evaluation-service";
import { Tier2Service } from "./services/tier2-service";
import { logger } from "./utils/logger";

// Initialize services
const qdrantService = new QdrantService();
const schemaService = new SchemaService();
const controlService = new ControlService();
const forensicAnalysisService = new ForensicAnalysisService(qdrantService);
const incidentService = new IncidentService();
const evaluationService = new EvaluationService();
const tier2Service = new Tier2Service(
	qdrantService,
	forensicAnalysisService,
	incidentService,
);
const tier2QueueService = new Tier2QueueService(tier2Service);

const app = new Hono();

app.onError((err, c) => {
	logger.error("Unhandled request error", err);
	return c.json({ error: "internal_error" }, 500);
});

app.notFound((c) => c.json({ error: "not_found" }, 404));

// Middleware to inject services
app.use("*", async (c, next) => {
	c.set("schemaService", schemaService);
	c.set("controlService", controlService);
	c.set("forensicAnalysisService", forensicAnalysisService);
	c.set("incidentService", incidentService);
	c.set("evaluationService", evaluationService);
	c.set("tier2QueueService", tier2QueueService);
	await next();
});

// Register routes
app.route("/", healthRoutes);
app.route("/control", controlRoutes);
app.route("/schema", schemaRoutes);
app.route("/analysis", analysisRoutes);
app.route("/", streamRoutes); // /tier2/anomalies
app.route("/evaluation", evaluationRoutes);

// Initialize application
async function initialize() {
	logger.info("Initializing VIA v2 Backend (Tier-2 Focus)");

	await initializeRegistry();
	await controlService.initialize();
	tier2QueueService.start();

	// Setup Qdrant collections
	await qdrantService.setupCollections();
	logger.info("Qdrant collections initialized");

	logger.info("VIA v2 Backend initialized successfully");
}

// Start server
async function startServer() {
	await initialize();

	const port = settings.server.port;
	const host = settings.server.host;

	logger.info("Starting server", { host, port });

	const server = Bun.serve({
		fetch: app.fetch,
		port,
		hostname: host,
		error: (error) => {
			logger.error("Bun server error", error);
			return new Response("internal_error", { status: 500 });
		},
	});

	logger.info("Server running", { url: server.url.toString() });

	const shutdown = (signal: string) => {
		logger.info("Shutting down gracefully", { signal });
		tier2QueueService.stop();
		server.stop();
		process.exit(0);
	};
	process.once("SIGINT", () => shutdown("SIGINT"));
	process.once("SIGTERM", () => shutdown("SIGTERM"));
}

// Start the server
startServer().catch((error) => {
	logger.error("Failed to start server", error);
	process.exit(1);
});

export {
	app,
	qdrantService,
	schemaService,
	controlService,
	forensicAnalysisService,
	incidentService,
	evaluationService,
	tier2Service,
	tier2QueueService,
};
