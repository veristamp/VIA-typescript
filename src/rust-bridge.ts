import type { Pointer } from "bun:ffi";
import { CString, dlopen, FFIType, ptr, suffix } from "bun:ffi";

// Default to debug for dev, can be switched to release
const libPath = `via-core/target/debug/via_core.${suffix}`;

const {
	symbols: { create_profile, free_profile, process_event, free_string },
} = dlopen(libPath, {
	// --- Anomaly Profile ---
	create_profile: {
		args: [
			FFIType.f64,
			FFIType.f64,
			FFIType.f64,
			FFIType.u64,
			FFIType.u64,
			FFIType.f64,
			FFIType.f64,
			FFIType.f64,
		],
		returns: FFIType.ptr,
	},
	free_profile: {
		args: [FFIType.ptr],
		returns: FFIType.void,
	},
	process_event: {
		args: [FFIType.ptr, FFIType.u64, FFIType.cstring, FFIType.f64, FFIType.ptr],
		returns: FFIType.void,
	},
	free_string: {
		args: [FFIType.cstring],
		returns: FFIType.void,
	},
});

type SimulationSymbols = {
	create_simulation: () => Pointer;
	free_simulation: (ptr: Pointer) => void;
	reset_simulation: (ptr: Pointer) => void;
	simulation_tick: (ptr: Pointer, deltaNs: bigint) => Pointer;
	add_normal_traffic: (ptr: Pointer, rps: number) => void;
	add_memory_leak: (ptr: Pointer, rateMbPerSec: number) => void;
	add_cpu_spike: (ptr: Pointer, intensity: number) => void;
	add_credential_stuffing: (ptr: Pointer, rps: number) => void;
	add_sql_injection: (ptr: Pointer, rps: number) => void;
	add_port_scan: (ptr: Pointer, speed: number) => void;
};

let simSymbols: SimulationSymbols | null = null;

function loadSimulationSymbols(): SimulationSymbols {
	if (simSymbols) return simSymbols;

	const simLibPath = `via-core/target/debug/via_sim.${suffix}`;
	try {
		const { symbols } = dlopen(simLibPath, {
			create_simulation: { args: [], returns: FFIType.ptr },
			free_simulation: { args: [FFIType.ptr], returns: FFIType.void },
			reset_simulation: { args: [FFIType.ptr], returns: FFIType.void },
			simulation_tick: {
				args: [FFIType.ptr, FFIType.u64],
				returns: FFIType.cstring,
			},
			add_normal_traffic: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
			add_memory_leak: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
			add_cpu_spike: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
			add_credential_stuffing: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
			add_sql_injection: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
			add_port_scan: {
				args: [FFIType.ptr, FFIType.f64],
				returns: FFIType.void,
			},
		});
		simSymbols = symbols as unknown as SimulationSymbols;
		return simSymbols;
	} catch {
		throw new Error(
			"Rust simulation FFI is unavailable. Build and expose simulation symbols, or disable simulation routes.",
		);
	}
}

const RESULT_SIZE = 40;

export class RustAnomalyProfile {
	private ptr: Pointer | null;
	private resultBuffer: ArrayBuffer;
	private resultView: DataView;
	private resultPtr: Pointer;

	constructor(config: {
		hw: { alpha: number; beta: number; gamma: number; period: number };
		hist: { bins: number; min: number; max: number; decay: number };
	}) {
		this.ptr = create_profile(
			config.hw.alpha,
			config.hw.beta,
			config.hw.gamma,
			config.hw.period,
			config.hist.bins,
			config.hist.min,
			config.hist.max,
			config.hist.decay,
		);
		this.resultBuffer = new ArrayBuffer(RESULT_SIZE);
		this.resultView = new DataView(this.resultBuffer);
		this.resultPtr = ptr(this.resultBuffer);
	}

	process(timestamp: number, uniqueId: string, value: number) {
		if (!this.ptr) throw new Error("Profile disposed");

		process_event(
			this.ptr,
			BigInt(timestamp),
			Buffer.from(`${uniqueId}\0`),
			value,
			this.resultPtr,
		);

		const isAnomaly = this.resultView.getUint8(0) !== 0;

		if (!isAnomaly) return null;

		return {
			isAnomaly: true,
			severity: this.resultView.getUint8(1),
			score: this.resultView.getFloat64(8, true),
			signalType: this.resultView.getUint8(16),
			expected: this.resultView.getFloat64(24, true),
			actual: this.resultView.getFloat64(32, true),
		};
	}

	dispose() {
		if (this.ptr) {
			free_profile(this.ptr);
			this.ptr = null;
		}
	}
}

export class RustSimulationEngine {
	private ptr: Pointer | null;
	private sim: SimulationSymbols;

	constructor() {
		this.sim = loadSimulationSymbols();
		this.ptr = this.sim.create_simulation();
	}

	reset() {
		if (!this.ptr) return;
		this.sim.reset_simulation(this.ptr);
	}

	addNormalTraffic(rps: number) {
		if (!this.ptr) return;
		this.sim.add_normal_traffic(this.ptr, rps);
	}

	addMemoryLeak(rateMbPerSec: number) {
		if (!this.ptr) return;
		this.sim.add_memory_leak(this.ptr, rateMbPerSec);
	}

	addCpuSpike(intensity: number) {
		if (!this.ptr) return;
		this.sim.add_cpu_spike(this.ptr, intensity);
	}

	addCredentialStuffing(rps: number) {
		if (!this.ptr) return;
		this.sim.add_credential_stuffing(this.ptr, rps);
	}

	addSqlInjection(rps: number) {
		if (!this.ptr) return;
		this.sim.add_sql_injection(this.ptr, rps);
	}

	addPortScan(speed: number) {
		if (!this.ptr) return;
		this.sim.add_port_scan(this.ptr, speed);
	}

	tick(deltaNs: number): string {
		if (!this.ptr) return "{}";

		// returns *mut c_char
		const jsonPtr = this.sim.simulation_tick(this.ptr, BigInt(deltaNs));

		const jsonStr = new CString(jsonPtr);
		const result = jsonStr.toString();

		// IMPORTANT: Free the string on Rust side to avoid leaks
		free_string(jsonPtr as unknown as Pointer);

		return result;
	}

	dispose() {
		if (this.ptr) {
			this.sim.free_simulation(this.ptr);
			this.ptr = null;
		}
	}
}
