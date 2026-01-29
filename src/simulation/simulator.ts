import type { IngestionService } from "../services/ingestion-service";
import type { EvaluationService } from "../services/evaluation-service";
import type { Scenario } from "./scenario";
import { CredentialStuffingScenario } from "./scenarios/credential-stuffing";

export class Simulator {
	private scenarios: Map<string, Scenario> = new Map();
	private activeScenario: Scenario | null = null;
	private intervalId: Timer | null = null;
	private ingestionService: IngestionService;
	private evaluationService: EvaluationService;
	private tick: number = 0;

	constructor(
		ingestionService: IngestionService,
		evaluationService: EvaluationService,
	) {
		this.ingestionService = ingestionService;
		this.evaluationService = evaluationService;

		// Register available scenarios
		this.registerScenario(new CredentialStuffingScenario());
	}

	private registerScenario(scenario: Scenario) {
		this.scenarios.set(scenario.getName(), scenario);
	}

	listScenarios() {
		return Array.from(this.scenarios.values()).map((s) => ({
			name: s.getName(),
			description: s.getDescription(),
		}));
	}

	startScenario(name: string) {
		if (this.activeScenario) {
			throw new Error(`Scenario ${this.activeScenario.getName()} is already running`);
		}

		const scenario = this.scenarios.get(name);
		if (!scenario) {
			throw new Error(`Scenario ${name} not found`);
		}

		console.log(`Starting simulation scenario: ${name}`);
		this.activeScenario = scenario;
		this.activeScenario.start();
		this.tick = 0;

		this.intervalId = setInterval(async () => {
			if (!this.activeScenario) return;

			this.tick++;
			const step = this.activeScenario.generateStep(this.tick);

			if (step) {
				// 1. Report Ground Truth
				this.evaluationService.recordGroundTruth({
					timestamp: step.timestamp,
					isAnomaly: !!step.metadata?.isAnomaly,
					scenarioName: name,
				});

				const logsToIngest: Record<string, unknown>[] = [];
				for (const log of step.logs) {
					const otlpLog = this.convertToOtlp(log);
					logsToIngest.push(otlpLog);
				}

				if (logsToIngest.length > 0) {
					// 2. Feed Logs to Ingestion
					await this.ingestionService.ingestLogBatch(logsToIngest);
				}
			}
		}, 1000); // 1 second tick
	}

	private convertToOtlp(simLog: Record<string, unknown>): Record<string, unknown> {
		const timestamp = (simLog.timestamp as number) * 1_000_000_000; // Nano
		const resourceAttrs = (simLog.resource as any)?.attributes || {};
		const formattedAttrs = Object.entries(resourceAttrs).map(([k, v]) => ({
			key: k,
			value: { stringValue: String(v) },
		}));

		return {
			resourceLogs: [
				{
					resource: {
						attributes: formattedAttrs,
					},
					scopeLogs: [
						{
							logRecords: [
								{
									timeUnixNano: String(timestamp),
									severityText: simLog.severityText,
									body: simLog.body,
									attributes: [],
								},
							],
						},
					],
				},
			],
		};
	}

	stopScenario() {
		if (this.activeScenario) {
			console.log(`Stopping simulation scenario: ${this.activeScenario.getName()}`);
			this.activeScenario.stop();
			this.activeScenario = null;
		}
		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = null;
		}
	}

    getStatus() {
        return {
            isRunning: !!this.activeScenario,
            activeScenario: this.activeScenario?.getName() || null,
            tick: this.tick
        };
    }
}
