import { dlopen, FFIType, suffix, ptr, CString, toArrayBuffer } from "bun:ffi";

// Default to debug for dev, can be switched to release
const libPath = `via-core/target/debug/via_core.${suffix}`; 

const {
  symbols: {
    // Anomaly Profile
    create_profile,
    free_profile,
    process_event,
    
    // Simulation Engine
    create_simulation,
    free_simulation,
    reset_simulation,
    simulation_tick,
    free_string,
    
    // Scenarios
    add_normal_traffic,
    add_memory_leak,
    add_cpu_spike,
    add_credential_stuffing,
    add_sql_injection,
    add_port_scan,
  },
} = dlopen(libPath, {
  // --- Anomaly Profile ---
  create_profile: {
    args: [
        FFIType.f64, FFIType.f64, FFIType.f64, FFIType.u64,
        FFIType.u64, FFIType.f64, FFIType.f64, FFIType.f64
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

  // --- Simulation Engine ---
  create_simulation: {
    args: [],
    returns: FFIType.ptr,
  },
  free_simulation: {
    args: [FFIType.ptr],
    returns: FFIType.void,
  },
  reset_simulation: {
    args: [FFIType.ptr],
    returns: FFIType.void,
  },
  simulation_tick: {
    args: [FFIType.ptr, FFIType.u64], // ptr, delta_ns
    returns: FFIType.cstring, // Returns JSON string pointer
  },
  free_string: {
    args: [FFIType.cstring],
    returns: FFIType.void,
  },

  // --- Scenarios ---
  add_normal_traffic: {
    args: [FFIType.ptr, FFIType.f64], // ptr, rps
    returns: FFIType.void,
  },
  add_memory_leak: {
    args: [FFIType.ptr, FFIType.f64], // ptr, rate_mb_sec
    returns: FFIType.void,
  },
  add_cpu_spike: {
    args: [FFIType.ptr, FFIType.f64], // ptr, intensity
    returns: FFIType.void,
  },
  add_credential_stuffing: {
    args: [FFIType.ptr, FFIType.f64], // ptr, rps
    returns: FFIType.void,
  },
  add_sql_injection: {
    args: [FFIType.ptr, FFIType.f64], // ptr, rps
    returns: FFIType.void,
  },
  add_port_scan: {
    args: [FFIType.ptr, FFIType.f64], // ptr, speed
    returns: FFIType.void,
  },
});

const RESULT_SIZE = 40;

export class RustAnomalyProfile {
  private ptr: number | null;
  private resultBuffer: ArrayBuffer;
  private resultView: DataView;
  private resultPtr: number;

  constructor(config: {
      hw: { alpha: number, beta: number, gamma: number, period: number },
      hist: { bins: number, min: number, max: number, decay: number }
  }) {
    this.ptr = create_profile(
        config.hw.alpha, config.hw.beta, config.hw.gamma, config.hw.period,
        config.hist.bins, config.hist.min, config.hist.max, config.hist.decay
    );
    this.resultBuffer = new ArrayBuffer(RESULT_SIZE);
    this.resultView = new DataView(this.resultBuffer);
    // @ts-ignore
    this.resultPtr = ptr(this.resultBuffer); 
  }

  process(timestamp: number, uniqueId: string, value: number) {
    if (!this.ptr) throw new Error("Profile disposed");
    
    process_event(
        this.ptr, 
        BigInt(timestamp), 
        Buffer.from(uniqueId + "\0"), 
        value, 
        this.resultPtr
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
      free_profile(this.ptr as any);
      this.ptr = null;
    }
  }
}

export class RustSimulationEngine {
    private ptr: number | null;

    constructor() {
        this.ptr = create_simulation();
    }

    reset() {
        if (!this.ptr) return;
        reset_simulation(this.ptr as any);
    }

    addNormalTraffic(rps: number) {
        if (!this.ptr) return;
        add_normal_traffic(this.ptr as any, rps);
    }

    addMemoryLeak(rateMbPerSec: number) {
        if (!this.ptr) return;
        add_memory_leak(this.ptr as any, rateMbPerSec);
    }

    addCpuSpike(intensity: number) {
        if (!this.ptr) return;
        add_cpu_spike(this.ptr as any, intensity);
    }

    addCredentialStuffing(rps: number) {
        if (!this.ptr) return;
        add_credential_stuffing(this.ptr as any, rps);
    }

    addSqlInjection(rps: number) {
        if (!this.ptr) return;
        add_sql_injection(this.ptr as any, rps);
    }

    addPortScan(speed: number) {
        if (!this.ptr) return;
        add_port_scan(this.ptr as any, speed);
    }

    tick(deltaNs: number): string {
        if (!this.ptr) return "{}";
        
        // returns *mut c_char
        const jsonPtr = simulation_tick(this.ptr as any, BigInt(deltaNs));
        
        // @ts-ignore: Bun CString wrapper
        const jsonStr = new CString(jsonPtr);
        const result = jsonStr.toString();
        
        // IMPORTANT: Free the string on Rust side to avoid leaks
        free_string(jsonPtr);
        
        return result;
    }

    dispose() {
        if (this.ptr) {
            free_simulation(this.ptr as any);
            this.ptr = null;
        }
    }
}
