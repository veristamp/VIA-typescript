import { Router } from "express";
import { RustSimulationEngine } from "../../core/rust-bridge";
import { z } from "zod";

const router = Router();
const simulation = new RustSimulationEngine();

// Start with some baseline traffic
simulation.addNormalTraffic(50.0);

const ScenarioSchema = z.object({
  type: z.enum([
    "normal", 
    "memory_leak", 
    "cpu_spike", 
    "credential_stuffing", 
    "sql_injection", 
    "port_scan"
  ]),
  intensity: z.number().positive(),
});

// GET /simulation/state
// Returns the latest batch of logs from the simulation
router.get("/tick", (req, res) => {
    // 100ms tick
    const logs = simulation.tick(100_000_000); 
    res.setHeader("Content-Type", "application/json");
    res.send(logs);
});

// POST /simulation/scenario
// Adds a new scenario to the running simulation
router.post("/scenario", (req, res) => {
    const result = ScenarioSchema.safeParse(req.body);
    if (!result.success) {
        return res.status(400).json(result.error);
    }

    const { type, intensity } = result.data;

    switch (type) {
        case "normal":
            simulation.addNormalTraffic(intensity);
            break;
        case "memory_leak":
            simulation.addMemoryLeak(intensity);
            break;
        case "cpu_spike":
            simulation.addCpuSpike(intensity); // 0.0 - 1.0
            break;
        case "credential_stuffing":
            simulation.addCredentialStuffing(intensity);
            break;
        case "sql_injection":
            simulation.addSqlInjection(intensity);
            break;
        case "port_scan":
            simulation.addPortScan(intensity);
            break;
    }

    res.json({ message: `Scenario ${type} added with intensity ${intensity}` });
});

// POST /simulation/reset
router.post("/reset", (req, res) => {
    simulation.reset();
    // Re-add baseline
    simulation.addNormalTraffic(50.0);
    res.json({ message: "Simulation reset to baseline" });
});

export default router;