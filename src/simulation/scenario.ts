export interface SimulationStep {
	timestamp: number;
	logs: Record<string, unknown>[];
	metadata?: Record<string, unknown>;
}

export abstract class Scenario {
	protected name: string;
	protected description: string;
	protected durationSeconds: number;
	protected isRunning: boolean = false;

	constructor(name: string, description: string, durationSeconds: number) {
		this.name = name;
		this.description = description;
		this.durationSeconds = durationSeconds;
	}

	abstract generateStep(tick: number): SimulationStep | null;

	start(): void {
		this.isRunning = true;
	}

	stop(): void {
		this.isRunning = false;
	}

	getName(): string {
		return this.name;
	}

	getDescription(): string {
		return this.description;
	}
}
