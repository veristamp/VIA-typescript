import { Scenario, type SimulationStep } from "../scenario";

export class CredentialStuffingScenario extends Scenario {
	constructor() {
		super(
			"credential_stuffing",
			"Simulates a brute-force login attack followed by account takeover attempts",
			300, // 5 minutes
		);
	}

	generateStep(tick: number): SimulationStep | null {
		if (!this.isRunning) return null;

		const now = Date.now() / 1000;
		const logs: Record<string, unknown>[] = [];
		const isAttackPhase = tick > 60 && tick < 240; // Attack happens between min 1 and 4

		// Background traffic
		for (let i = 0; i < 5; i++) {
			logs.push({
				timestamp: now,
				severityText: "INFO",
				body: { stringValue: "User login successful" },
				resource: {
					attributes: {
						"service.name": "auth-service",
						"host.name": `web-${Math.floor(Math.random() * 3) + 1}`,
					},
				},
			});
		}

		// Attack traffic
		if (isAttackPhase) {
			const attackIntensity = Math.floor(Math.random() * 50) + 20; // 20-70 reqs/sec
			for (let i = 0; i < attackIntensity; i++) {
				logs.push({
					timestamp: now,
					severityText: "WARN",
					body: { stringValue: "Login failed: Invalid credentials" },
					resource: {
						attributes: {
							"service.name": "auth-service",
							"client.ip": `192.168.1.${Math.floor(Math.random() * 255)}`,
						},
					},
				});
			}
		}

		return {
			timestamp: now,
			logs,
			metadata: {
				isAnomaly: isAttackPhase,
				anomalyType: "frequency_spike",
			},
		};
	}
}
