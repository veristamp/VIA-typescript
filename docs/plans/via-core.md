Now I have your preferences. Let me write the comprehensive implementation plan.

---

# VIA-Core v2: Enterprise Anomaly Detection Engine - Implementation Plan

## Executive Summary

Transform via-core from a high-performance but rigid detection engine into a **configurable, observable, and recoverable** enterprise-grade anomaly detection system. This plan maintains the existing 1.1M+ EPS performance while adding dynamic configuration, full explainability, memory management, and schema validation.

**Target Architecture:**
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Configuration Layer                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │  TOML Config │  │ Schema       │  │ Hot-Reload   │  │ Model Storage  │  │
│  │  Files       │  │ Validator    │  │ Coordinator  │  │ (Checkpoint)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └───────┬────────┘  │
└─────────┼─────────────────┼─────────────────┼───────────────────┼───────────┘
          │                 │                 │                   │
          ▼                 ▼                 ▼                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Core Detection Engine                                 │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                    Sharded AnomalyProcessor                           │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐         ┌─────────────────┐   │   │
│  │  │ Shard 0 │  │ Shard 1 │  │ Shard 2 │  ...    │ Memory Manager  │   │   │
│  │  │ (Lock-Free Queue + Ensemble Detector)                            │   │   │
│  │  └─────────┘  └─────────┘  └─────────┘         └─────────────────┘   │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                    ↑                              ↑                          │
│              ┌─────┴─────┐                 ┌──────┴──────┐                   │
│              │ SIMD-JSON │                 │  Explainable│                   │
│              │  Parser   │                 │  Anomaly    │                   │
│              └───────────┘                 │  Result     │                   │
│                                            └─────────────┘                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Phase 1: Foundation (Weeks 1-2)

### 1.1 Schema Service (`crates/via-schema`)

**Purpose:** Single source of truth for all configuration validation.

```rust
// crates/via-schema/src/lib.rs

pub struct AnomalyDetectorConfig {
    pub enabled: bool,
    pub detector_type: DetectorType,
    pub parameters: HashMap<String, Value>,
    pub thresholds: ThresholdConfig,
    pub window_size: Duration,
    pub warmup_events: usize,
}

pub enum DetectorType {
    VolumeDetectorV2(VolumeConfig),
    DistributionDetectorV2(DistributionConfig),
    CardinalityDetectorV2(CardinalityConfig),
    BurstDetectorV2(BurstConfig),
    SpectralDetector(SpectralConfig),
    ChangePointDetector(ChangePointConfig),
    RRCFDetector(RRCFConfig),
    MultiScaleDetector(MultiScaleConfig),
    BehavioralFingerprint(BehavioralConfig),
    DriftDetector(DriftConfig),
}

pub struct ThresholdConfig {
    pub method: ThresholdMethod,
    pub sensitivity: f64,           // 0.0 - 1.0
    pub adaptive_factor: f64,       // Auto-tune multiplier
    pub manual_values: Option<ManualThresholds>,
}

pub enum ThresholdMethod {
    EWMA_sigma,
    Percentile(f64),               // e.g., 99.5
    MAD,                           // Median Absolute Deviation
    Ensemble,
}

pub struct VolumeConfig {
    pub hw_alpha: f64,             // Holt-Winters smoothing
    pub hw_beta: f64,
    pub hw_gamma: f64,
    pub period: usize,             // Seasonality period
    pub seasonality_strength: f64, // Auto-detect strength
}

pub struct DistributionConfig {
    pub histogram_bins: usize,
    pub min_value: f64,
    pub max_value: f64,
    pub decay_factor: f64,
    pub bin_strategy: BinStrategy,
}

pub struct CardinalityConfig {
    pub hll_precision: u8,         // 10-18
    pub velocity_window: Duration,
    pub baseline_samples: usize,
}

pub struct BehavioralConfig {
    pub fingerprint_features: Vec<FingerprintFeature>,
    pub comparison_window: Duration,
    pub drift_tolerance: f64,
}
```

**Key Features:**
- JSON Schema export for TypeScript/Bun clients
- Runtime validation with detailed error messages
- Config versioning for migrations
- Default values with documentation

### 1.2 Configuration Service (`crates/via-config`)

**Purpose:** Load, validate, and manage configuration lifecycle.

```rust
// crates/via-config/src/lib.rs

pub struct ConfigService {
    schema: SchemaService,
    sources: Vec<ConfigSource>,           // File, ENV, Runtime API
    hot_reload_channels: HashMap<ConfigPath, Arc<RwLock<ConfigSubscriber>>>,
    model_storage: ModelStorage,
}

impl ConfigService {
    pub async fn load(&self, path: PathBuf) -> Result<ValidatedConfig, ConfigError>;
    pub async fn hot_reload(&self, path: PathBuf) -> Result<(), ConfigError>;
    pub fn get_detector_config(&self, entity_id: u64) -> Option<&AnomalyDetectorConfig>;
    pub async fn update_threshold(&self, detector_id: &str, threshold: f64) -> Result<()>;
    pub fn subscribe(&self, key: &str) -> impl Stream<Item = ConfigChange>;
}
```

**Hybrid Hot-Reload Strategy:**

| Config Type | Hot-Reload Support | Mechanism |
|------------|-------------------|-----------|
| Detector parameters | Yes | Signal channel to workers |
| Threshold values | Yes | Atomic swap, current event uses old |
| Ensemble weights | Yes | Gradual transition |
| Profile schema changes | No | Requires restart with graceful drain |
| Memory limits | Yes | Immediate enforcement |

### 1.3 TOML Configuration Schema

```toml
# via-core.toml

[engine]
profile_path = "/var/lib/via-core/profiles"
max_memory_mb = 4096
memory_pressure_threshold = 0.85
shutdown_grace_period = "30s"

[detectors.volume]
enabled = true
hw_alpha = 0.1
hw_beta = 0.05
hw_gamma = 0.1
period = 60
seasonality_auto_detect = true

[detectors.volume.threshold]
method = "ensemble"
sensitivity = 0.7
adaptive_factor = 1.0

[detectors.distribution]
enabled = true
histogram_bins = 50
min_value = 0.0
max_value = 5000.0
decay_factor = 0.99

[detectors.cardinality]
enabled = true
hll_precision = 12
velocity_window = "60s"
baseline_samples = 1000

[detectors.spectral]
enabled = true
salience_threshold = 2.0
window_size = 128

[detectors.behavioral]
enabled = true
features = ["mean", "stddev", "skewness", "kurtosis"]
comparison_window = "1h"
drift_tolerance = 0.1

[thresholds]
volume = 0.8
distribution = 0.75
cardinality = 0.85
spectral = 0.7
behavioral = 0.8
overall_anomaly = 0.7

[persistence]
enabled = true
checkpoint_interval = "5m"
storage_path = "/var/lib/via-core/checkpoints"
max_checkpoints = 10

[observability]
prometheus_port = 9090
log_level = "info"
explainability = "full"  # none, basic, full
trace_sampling_rate = 0.1
```

## Phase 2: Core Engine Enhancements (Weeks 3-4)

### 2.1 Memory Pressure Manager

**Purpose:** Prevent OOM while maintaining detection quality.

```rust
// crates/via-core/src/memory/manager.rs

pub struct MemoryManager {
    pressure_observer: PressureObserver,
    eviction_policy: EvictionPolicy,
    profile_registry: ProfileRegistry,
    metrics: MemoryMetrics,
}

pub enum EvictionPolicy {
    LRU {
        max_entries: usize,
        min_access_frequency: f64,
    },
    TTL {
        max_idle_duration: Duration,
        heartbeat_interval: Duration,
    },
    Hybrid {
        max_memory_mb: usize,
        lru_weight: f64,
        ttl_weight: f64,
    },
    PriorityBased {
        min_priority: u8,
        max_size_per_priority: HashMap<u8, usize>,
    },
}

impl MemoryManager {
    pub fn try_acquire(&self, entity_id: u64) -> Result<(), MemoryRejection> {
        let current = self.metrics.allocated_bytes();
        if current >= self.config.max_memory_mb * 1024 * 1024 {
            self.handle_memory_pressure();
        }
        // ... allocate profile
    }

    fn handle_memory_pressure(&self) {
        match self.config.pressure_strategy {
            PressureStrategy::EvictStale => self.evict_stale_profiles(),
            PressureStrategy::RejectNew => return Err(MemoryExhausted),
            PressureStrategy::DowngradeSensitivity => self.reduce_detection_sensitivity(),
            PressureStrategy::CompressOldest => self.compress_historical_profiles(),
        }
    }
}

pub struct ProfileRegistry {
    entries: DashMap<u64, Arc<ProfileEntry>>,
    access_order: LinkedHashMap<u64, Instant>,
    memory_budget: AtomicUsize,
}
```

**Pressure Levels:**
- **Level 0 (0-70%):** Normal operation
- **Level 1 (70-85%):** Start evicting stale profiles, reject very low-priority entities
- **Level 2 (85-95%):** Reduce ensemble sensitivity, aggressive eviction
- **Level 3 (95%+):** Fail-safe mode, only high-priority detection

### 2.2 Explainable Anomaly Result

**Purpose:** Full SHAP-like attribution for why an anomaly was detected.

```rust
// crates/via-core/src/explain.rs

pub struct ExplainableAnomaly {
    pub entity_id: u64,
    pub timestamp: u64,
    pub value: f64,
    
    pub overall_score: f64,              // 0.0 - 1.0
    pub is_anomaly: bool,
    pub severity: Severity,
    
    pub attribution: DetectorAttribution,
    pub contextual_factors: Vec<ContextFactor>,
    pub counterfactual: Counterfactual,
    pub recommended_actions: Vec<Action>,
}

pub struct DetectorAttribution {
    pub detector_type: String,
    pub detector_score: f64,
    pub contribution_to_overall: f64,     // SHAP-like value
    pub features: Vec<FeatureContribution>,
    pub expected_value: f64,
    pub deviation_magnitude: f64,
    pub deviation_direction: Direction,
    pub historical_context: HistoricalContext,
}

pub struct FeatureContribution {
    pub feature_name: String,
    pub contribution: f64,               // Positive = pushing toward anomaly
    pub raw_value: f64,
    pub expected_range: (f64, f64),
    pub percentile: f64,
}

pub struct Counterfactual {
    pub description: String,
    pub threshold_value: f64,
    pub minimal_change_required: f64,
    pub probability_after_change: f64,
}

pub struct ContextFactor {
    pub factor_type: FactorType,
    pub description: String,
    pub relevance_score: f64,            // How much this context matters
    pub current_context: Value,
    pub historical_context: Value,
}

pub enum FactorType {
    TimeOfDay,
    DayOfWeek,
    BusinessHours,
    RecentTrend,
    SeasonalPattern,
    ExternalEvent,
}

pub struct HistoricalContext {
    pub similar_anomalies: Vec<AnomalyReference>,
    pub average_baseline: f64,
    pub variance_baseline: f64,
    pub trend_direction: Trend,
}
```

**Example Output:**
```json
{
  "entity_id": "123456789",
  "timestamp": 1699900800,
  "value": 450.2,
  "overall_score": 0.92,
  "is_anomaly": true,
  "severity": "high",
  "attribution": {
    "detector": "DistributionDetectorV2",
    "detector_score": 0.89,
    "contribution_to_overall": 0.85,
    "expected_value": 120.5,
    "deviation_magnitude": 274.2,
    "deviation_direction": "above",
    "features": [
      {
        "feature_name": "latency_p99",
        "contribution": 0.65,
        "raw_value": 450.2,
        "expected_range": [80.0, 180.0],
        "percentile": 99.8
      },
      {
        "feature_name": "histogram_mass_shift",
        "contribution": 0.24,
        "raw_value": 0.73,
        "expected_range": [0.0, 0.2],
        "percentile": 99.5
      }
    ]
  },
  "counterfactual": {
    "description": "If latency were below 210.5ms, anomaly probability drops to 0.15",
    "threshold_value": 210.5,
    "minimal_change_required": "-53%",
    "probability_after_change": 0.15
  },
  "contextual_factors": [
    {
      "factor_type": "TimeOfDay",
      "description": "Anomaly occurred during peak traffic hours (14:00-16:00)",
      "relevance_score": 0.3,
      "current_context": "peak",
      "historical_context": "typical"
    }
  ],
  "recommended_actions": [
    "Check for upstream service degradation",
    "Review recent deployment changes",
    "Investigate potential DoS attack pattern"
  ]
}
```

### 2.3 Model Persistence & Recovery

**Purpose:** Save/restore learned profiles for zero warmup after restart.

```rust
// crates/via-core/src/persistence/mod.rs

pub struct ModelStorage {
    storage_dir: PathBuf,
    checkpoint_interval: Duration,
    max_checkpoints: usize,
    encoder: BincodeEncoder,
    compression: Lz4,
}

impl ModelStorage {
    pub async fn checkpoint(&self, profiles: &HashMap<u64, AnomalyProfile>) -> Result<CheckpointId> {
        let checkpoint = Checkpoint {
            id: Uuid::new_v4(),
            timestamp: Instant::now(),
            version: VERSION,
            profiles: profiles
                .iter()
                .filter(|(id, p)| p.should_persist())
                .map(|(id, p)| (id, p.serialize()))
                .collect(),
            metadata: self.collect_metrics(),
        };
        
        // Atomic write with rename
        let temp_path = self.storage_dir.join(format!("checkpoint_{}.tmp", checkpoint.id));
        self.encode_to_file(&checkpoint, &temp_path).await?;
        fs::rename(&temp_path, self.path_for_id(checkpoint.id)).await?;
        
        // Maintain only N checkpoints
        self.prune_old_checkpoints().await?;
        
        Ok(checkpoint.id)
    }
}

pub struct RecoveryManager {
    storage: ModelStorage,
    recovery_strategy: RecoveryStrategy,
}

pub enum RecoveryStrategy {
    Full,                    // Load all profiles from checkpoint
    PriorityBased(usize),    // Only load top N by priority
    Adaptive {
        min_entities: usize,
        max_recovery_time: Duration,
    },
}

impl RecoveryManager {
    pub async fn recover(&self) -> Result<HashMap<u64, AnomalyProfile>, RecoveryError> {
        let latest = self.storage.latest_checkpoint().await?;
        if let Some(checkpoint) = latest {
            info!("Recovering {} profiles from checkpoint {}", 
                  checkpoint.profiles.len(), checkpoint.id);
            
            let profiles = match self.recovery_strategy {
                RecoveryStrategy::Full => self.load_all(&checkpoint).await,
                RecoveryStrategy::PriorityBased(n) => self.load_by_priority(&checkpoint, n).await,
                RecoveryStrategy::Adaptive { .. } => self.load_adaptive(&checkpoint).await,
            }?;
            
            info!("Recovered {} profiles", profiles.len());
            Ok(profiles)
        } else {
            info!("No checkpoint found, starting fresh");
            Ok(HashMap::new())
        }
    }
}
```

## Phase 3: Dynamic Configuration API (Weeks 5-6)

### 3.1 Runtime Control gRPC Service

```protobuf
service ControlService {
  // Configuration management
  rpc GetConfig(GetConfigRequest) returns (GetConfigResponse);
  rpc UpdateConfig(UpdateConfigRequest) returns (UpdateConfigResponse);
  rpc HotReloadConfig(HotReloadRequest) returns (HotReloadResponse);
  
  // Entity management
  rpc GetEntityProfile(EntityRequest) returns (EntityProfileResponse);
  rpc ResetEntityProfile(EntityRequest) returns (ResetResponse);
  rpc SetEntityPriority(EntityPriorityRequest) returns (ResetResponse);
  
  // Model management
  rpc CreateCheckpoint(CheckpointRequest) returns (CheckpointResponse);
  rpc ListCheckpoints(ListRequest) returns (ListResponse);
  rpc TriggerRecovery(RecoveryRequest) returns (RecoveryResponse);
  
  // Observability
  rpc GetMetrics(MetricsRequest) returns (MetricsResponse);
  rpc GetDetectorState(DetectorStateRequest) returns (DetectorStateResponse);
  rpc ExplainAnomaly(ExplainRequest) returns (ExplainResponse);
  
  // Admin
  rpc Shutdown(ShutdownRequest) returns (ShutdownResponse);
  rpc HealthCheck(HealthRequest) returns (HealthResponse);
}
```

### 3.2 Control Client (for TypeScript/Bun Integration)

```rust
// crates/via-control/src/lib.rs

pub struct ViaControlClient {
    channel: Channel,
    stub: ControlServiceClient,
}

impl ViaControlClient {
    pub async fn update_threshold(
        &self,
        detector: &str,
        entity_id: Option<u64>,
        threshold: f64,
    ) -> Result<UpdateResult> {
        let mut req = UpdateConfigRequest::default();
        req.config = Some(ConfigUpdate {
            target: ConfigTarget::DetectorThreshold(detector.to_string()),
            entity_id,
            value: serde_json::to_value(threshold)?,
            hot_reload: true,
        });
        
        let response = self.stub.update_config(&req).await?;
        Ok(response.result)
    }

    pub async fn explain_anomaly(&self, anomaly_id: &str) -> Result<ExplainableAnomaly> {
        let mut req = ExplainRequest::default();
        req.anomaly_id = anomaly_id.to_string();
        req.detail_level = DetailLevel::Full as i32;
        
        let response = self.stub.explain_anomaly(&req).await?;
        Ok(response.explanation)
    }
}
```

## Phase 4: Performance Optimizations (Weeks 7-8)

### 4.1 Adaptive Ensemble

```rust
// crates/via-core/src/ensemble/adaptive.rs

pub struct AdaptiveEnsemble {
    detectors: Vec<Box<dyn Detector>>,
    meta_learner: MetaLearner,
    window_size: usize,
    update_interval: Duration,
    weights: AtomicVec<f64>,
}

pub struct MetaLearner {
    model: OnlineLogisticRegression,
    feature_extractor: EnsembleFeatureExtractor,
}

impl AdaptiveEnsemble {
    pub fn process(&self, event: &Event) -> AnomalyResult {
        // Parallel detector execution
        let (scores, explanations): (Vec<_>, Vec<_>) = self.detectors
            .par_iter()
            .map(|d| d.process(event))
            .collect();
        
        // Weighted combination
        let weights = self.weights.load(Ordering::Relaxed);
        let combined = self.combine_scores(&scores, &weights);
        
        // Meta-learner correction
        let features = self.feature_extractor.extract(&scores, &event);
        let correction = self.meta_learner.predict(&features);
        
        let final_score = (combined + correction).clamp(0.0, 1.0);
        
        AnomalyResult {
            score: final_score,
            contributions: self.compute_contributions(&scores, &weights),
            explanation: self.explain(&scores, &explanations),
            ..Default::default()
        }
    }
}
```

### 4.2 SIMD-Optimized Critical Paths

```rust
// crates/via-core/src/algo/simd_ops.rs

#[target_feature(enable = "avx2")]
unsafe fn compute_attribution_avx2(
    scores: &[f64],
    weights: &[f64],
    output: &mut [f64],
) {
    let n = scores.len();
    let mut i = 0;
    let chunk = 4;
    
    // Process 4 elements at a time
    while i + chunk <= n {
        let scores_vec = _mm256_loadu_pd(scores.as_ptr().add(i));
        let weights_vec = _mm256_loadu_pd(weights.as_ptr().add(i));
        let result = _mm256_mul_pd(scores_vec, weights_vec);
        _mm256_storeu_pd(output.as_ptr().add(i), result);
        i += chunk;
    }
    
    // Handle remainder
    while i < n {
        output[i] = scores[i] * weights[i];
        i += 1;
    }
}
```

## Phase 5: Testing & Documentation (Weeks 9-10)

### 5.1 Test Strategy

| Test Type | Coverage Target | Tools |
|-----------|----------------|-------|
| Unit Tests | >95% functions | cargo test |
| Integration Tests | All public APIs | rstest, proptest |
| Performance Tests | Baseline regression | criterion,benchmarks |
| Chaos Tests | Graceful degradation | custom chaos framework |
| Property Tests | Algorithm correctness | proptest |
| FFI Tests | C/TypeScript bindings | ctest, ts-jest |

### 5.2 Documentation Structure

```
docs/
├── architecture/
│   ├── overview.md
│   ├── data-flow.md
│   ├── detector-reference.md
│   └── configuration.md
├── user-guide/
│   ├── quickstart.md
│   ├── configuration.md
│   └── api-reference.md
├── developer-guide/
│   ├── adding-detectors.md
│   ├── contributing.md
│   └── testing.md
└── benchmarks/
    └── performance-results.md
```

## Implementation Roadmap

```
Week 1-2: Foundation
├── via-schema (JSON Schema + Rust types)
├── via-config (TOML loader + validation)
└── Hot-reload infrastructure

Week 3-4: Core Enhancements
├── Memory pressure manager
├── ExplainableAnomaly struct
├── Model persistence
└── Checkpoint/recovery

Week 5-6: Control API
├── gRPC ControlService
├── Control client library
└── Admin CLI

Week 7-8: Performance
├── Adaptive ensemble
├── SIMD optimizations
└── Profile compression

Week 9-10: Testing & Polish
├── Integration tests
├── Documentation
└── Release prep
```

## Migration Strategy

**Backward Compatibility:**
- FFI interface unchanged (add optional parameters)
- Default values for new config fields
- Graceful degradation for missing features

**Upgrade Path:**
1. Update to v2.x with existing config (warnings for deprecated fields)
2. New config format optional, fallback to old
3. Migrate profiles on first checkpoint
4. Hot-reload for threshold changes during migration

## Key Metrics & Success Criteria

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| Throughput | 1.1M EPS | 1M+ EPS | Benchmark suite |
| P99 Latency | <5ms | <5ms | Production monitoring |
| Memory growth | Unbounded | Configurable cap | Memory manager |
| Explainability | None | Full SHAP-like | Feature complete |
| Config hot-reload | No | Hybrid | Integration tests |
| Model recovery | None | <30s warmup | Recovery test |

---

**Ready to proceed with implementation?**

I recommend starting with Phase 1 (Schema + Config) as it enables all subsequent work. Shall I begin writing the actual code for the Schema Service first?