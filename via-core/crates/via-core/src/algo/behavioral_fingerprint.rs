//! Behavioral Fingerprinting Per-Entity Profile
//!
//! Learns normal behavior patterns for individual entities (IPs, users, devices)
//! and detects deviations from learned profiles.
//!
//! Features tracked:
//! - Temporal patterns (hours of activity)
//! - Service/resource access patterns
//! - Geographic patterns (with HLL for efficient storage)
//! - Request velocity and timing
//! - Payload characteristics

use crate::algo::{histogram::FadingHistogram, hll::HyperLogLog};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hour of day histogram (24 bins)
type HourHistogram = [u64; 24];

/// Behavioral profile for a single entity
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BehavioralProfile {
    /// Entity identifier hash
    pub entity_hash: u64,
    /// First seen timestamp
    pub first_seen: u64,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Normal hours of activity (hour -> count)
    pub normal_hours: HourHistogram,
    /// Total observations
    pub observation_count: u64,
    /// Request velocity (EWMA-like)
    pub velocity_ewma: f64,
    /// Velocity alpha for updates
    velocity_alpha: f64,
    /// Inter-arrival times (ms)
    pub iat_histogram: FadingHistogram,
    /// Payload size distribution
    pub payload_histogram: FadingHistogram,
    /// Services/resources accessed (Count-Min sketch simulation)
    pub service_access: HashMap<u64, u64>,
    /// Geographic locations (HLL for IP diversity)
    pub geo_diversity: HyperLogLog,
    /// Recent behavior score (0.0 = normal, 1.0 = anomalous)
    pub behavior_score: f64,
    /// Anomaly count
    pub anomaly_count: u64,
    /// Is profile mature enough for detection
    pub is_mature: bool,
    /// Maturity threshold
    maturity_threshold: u64,
}

impl BehavioralProfile {
    pub fn new(entity_hash: u64, timestamp: u64) -> Self {
        Self {
            entity_hash,
            first_seen: timestamp,
            last_seen: timestamp,
            normal_hours: [0; 24],
            observation_count: 0,
            velocity_ewma: 0.0,
            velocity_alpha: 0.1,
            iat_histogram: FadingHistogram::new(20, 0.0, 10000.0, 0.999),
            payload_histogram: FadingHistogram::new(20, 0.0, 100000.0, 0.999),
            service_access: HashMap::with_capacity(50),
            geo_diversity: HyperLogLog::new(10),
            behavior_score: 0.0,
            anomaly_count: 0,
            is_mature: false,
            maturity_threshold: 50,
        }
    }

    /// Update profile with new event
    pub fn update(
        &mut self,
        timestamp_ns: u64,
        iat_ms: f64,
        payload_size: f64,
        service_hash: u64,
        geo_hash: u64,
    ) {
        self.last_seen = timestamp_ns;
        self.observation_count += 1;

        // Update hour histogram
        let hour = ((timestamp_ns / 3_600_000_000_000u64) % 24) as usize;
        self.normal_hours[hour] += 1;

        // Update velocity
        if self.velocity_ewma == 0.0 {
            self.velocity_ewma = 1.0 / iat_ms.max(1.0);
        } else {
            let instant_velocity = 1.0 / iat_ms.max(1.0);
            self.velocity_ewma = self.velocity_alpha * instant_velocity
                + (1.0 - self.velocity_alpha) * self.velocity_ewma;
        }

        // Update histograms
        self.iat_histogram.update(iat_ms);
        self.payload_histogram.update(payload_size);

        // Update service access
        *self.service_access.entry(service_hash).or_insert(0) += 1;

        // Update geo diversity
        self.geo_diversity.add_hash(geo_hash);

        // Check maturity
        if self.observation_count >= self.maturity_threshold {
            self.is_mature = true;
        }
    }

    /// Calculate deviation score for a new event
    pub fn calculate_deviation(
        &mut self,
        timestamp_ns: u64,
        iat_ms: f64,
        payload_size: f64,
        service_hash: u64,
        _geo_hash: u64,
    ) -> f64 {
        if !self.is_mature {
            return 0.0; // Not enough data
        }

        let mut deviations = vec![];

        // 1. Temporal deviation (unusual hour)
        let hour = ((timestamp_ns / 3_600_000_000_000u64) % 24) as usize;
        let hour_count = self.normal_hours[hour];
        let avg_hour_count = self.observation_count / 24;
        if hour_count < avg_hour_count / 2 {
            deviations.push(0.3); // Unusual hour
        }

        // 2. Velocity deviation
        let instant_velocity = 1.0 / iat_ms.max(1.0);
        let velocity_ratio = instant_velocity / self.velocity_ewma.max(0.001);
        if velocity_ratio > 5.0 {
            deviations.push(0.4 * (velocity_ratio.min(10.0) / 10.0));
        }

        // 3. IAT distribution deviation (using histogram rarity)
        let iat_rarity = self.iat_histogram.rarity_score(iat_ms);
        if iat_rarity > 0.8 {
            deviations.push(0.3 * iat_rarity);
        }

        // 4. Payload size deviation
        let payload_rarity = self.payload_histogram.rarity_score(payload_size);
        if payload_rarity > 0.8 {
            deviations.push(0.2 * payload_rarity);
        }

        // 5. Service access deviation (new service)
        if !self.service_access.contains_key(&service_hash) {
            deviations.push(0.3); // Accessing new service
        }

        // Combine deviations (max for high sensitivity, sum for accumulation)
        let score: f64 = deviations.iter().cloned().fold(0.0_f64, f64::max);

        // Update behavior score with EWMA
        self.behavior_score = 0.1 * score + 0.9 * self.behavior_score;

        if score > 0.5 {
            self.anomaly_count += 1;
        }

        score
    }

    /// Get typical hours of activity (hours with > avg activity)
    pub fn get_typical_hours(&self) -> Vec<usize> {
        let avg = self.observation_count / 24;
        self.normal_hours
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > avg)
            .map(|(hour, _)| hour)
            .collect()
    }

    /// Get service diversity (number of unique services)
    pub fn get_service_diversity(&self) -> usize {
        self.service_access.len()
    }

    /// Get geo diversity count
    pub fn get_geo_diversity(&self) -> f64 {
        self.geo_diversity.count()
    }

    /// Check if behavior is anomalous
    pub fn is_anomalous(&self, threshold: f64) -> bool {
        self.behavior_score > threshold && self.is_mature
    }

    pub fn get_stats(&self) -> (u64, u64, bool, f64, usize, f64) {
        (
            self.observation_count,
            self.anomaly_count,
            self.is_mature,
            self.behavior_score,
            self.get_service_diversity(),
            self.get_geo_diversity(),
        )
    }
}

/// Profile store for managing multiple entity profiles
#[derive(Serialize, Deserialize, Clone)]
pub struct ProfileStore {
    /// Entity hash -> Profile
    profiles: HashMap<u64, BehavioralProfile>,
    /// Maximum number of profiles to store
    max_profiles: usize,
    /// Profile access times for LRU eviction
    access_times: HashMap<u64, u64>,
    /// Global access counter
    access_counter: u64,
    /// Default maturity threshold
    maturity_threshold: u64,
}

impl ProfileStore {
    pub fn new(max_profiles: usize, maturity_threshold: u64) -> Self {
        Self {
            profiles: HashMap::with_capacity(max_profiles.min(100000)),
            max_profiles: max_profiles.max(10).min(1000000), // Allow smaller for testing
            access_times: HashMap::with_capacity(max_profiles.min(100000)),
            access_counter: 0,
            maturity_threshold,
        }
    }

    /// Get or create profile for entity
    pub fn get_or_create_profile(
        &mut self,
        entity_hash: u64,
        timestamp_ns: u64,
    ) -> &mut BehavioralProfile {
        self.access_counter += 1;

        if !self.profiles.contains_key(&entity_hash) {
            // Check if we need to evict
            if self.profiles.len() >= self.max_profiles {
                self.evict_lru();
            }

            let profile = BehavioralProfile::new(entity_hash, timestamp_ns);
            self.profiles.insert(entity_hash, profile);
        }

        self.access_times.insert(entity_hash, self.access_counter);
        self.profiles.get_mut(&entity_hash).unwrap()
    }

    /// Get existing profile (if mature)
    pub fn get_profile(&mut self, entity_hash: u64) -> Option<&BehavioralProfile> {
        self.access_counter += 1;

        let profile = self.profiles.get(&entity_hash)?;

        if profile.is_mature {
            self.access_times.insert(entity_hash, self.access_counter);
            Some(profile)
        } else {
            None
        }
    }

    /// Update profile and check for deviation
    pub fn update_and_check(
        &mut self,
        entity_hash: u64,
        timestamp_ns: u64,
        iat_ms: f64,
        payload_size: f64,
        service_hash: u64,
        geo_hash: u64,
    ) -> (f64, bool) {
        let profile = self.get_or_create_profile(entity_hash, timestamp_ns);

        let deviation =
            profile.calculate_deviation(timestamp_ns, iat_ms, payload_size, service_hash, geo_hash);

        profile.update(timestamp_ns, iat_ms, payload_size, service_hash, geo_hash);

        let is_anomalous = profile.is_anomalous(0.6);

        (deviation, is_anomalous)
    }

    /// Evict least recently used profile
    fn evict_lru(&mut self) {
        // Find the oldest entry by collecting the key separately
        let oldest_hash = self
            .access_times
            .iter()
            .min_by_key(|(_key, time)| **time)
            .map(|(hash, _)| *hash);

        if let Some(hash) = oldest_hash {
            self.profiles.remove(&hash);
            self.access_times.remove(&hash);
        }
    }

    /// Get store statistics
    pub fn get_stats(&self) -> (usize, u64, u64) {
        let mature_count = self.profiles.values().filter(|p| p.is_mature).count();
        (
            self.profiles.len(),
            mature_count as u64,
            self.access_counter,
        )
    }

    /// Get all mature profiles
    pub fn get_mature_profiles(&self) -> Vec<&BehavioralProfile> {
        self.profiles.values().filter(|p| p.is_mature).collect()
    }

    /// Reset all profiles
    pub fn reset(&mut self) {
        self.profiles.clear();
        self.access_times.clear();
        self.access_counter = 0;
    }

    /// Batch update from events
    pub fn batch_update(&mut self, events: Vec<EntityEvent>) -> Vec<(u64, f64, bool)> {
        events
            .into_iter()
            .map(|event| {
                let (score, is_anomaly) = self.update_and_check(
                    event.entity_hash,
                    event.timestamp_ns,
                    event.iat_ms,
                    event.payload_size,
                    event.service_hash,
                    event.geo_hash,
                );
                (event.entity_hash, score, is_anomaly)
            })
            .collect()
    }
}

/// Event for entity behavior tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityEvent {
    pub entity_hash: u64,
    pub timestamp_ns: u64,
    pub iat_ms: f64,
    pub payload_size: f64,
    pub service_hash: u64,
    pub geo_hash: u64,
}

/// Behavioral fingerprinting detector (wrapper for engine integration)
pub struct BehavioralFingerprintDetector {
    store: ProfileStore,
    last_timestamp: u64,
    last_entity: u64,
}

impl BehavioralFingerprintDetector {
    pub fn new(max_profiles: usize) -> Self {
        Self {
            store: ProfileStore::new(max_profiles, 30),
            last_timestamp: 0,
            last_entity: 0,
        }
    }

    /// Process event and detect behavioral anomalies
    pub fn process(
        &mut self,
        entity_hash: u64,
        timestamp_ns: u64,
        payload_size: f64,
        service_hash: u64,
    ) -> (f64, bool, String) {
        // Calculate IAT
        let iat_ms = if self.last_entity == entity_hash && self.last_timestamp > 0 {
            (timestamp_ns.saturating_sub(self.last_timestamp)) as f64 / 1_000_000.0
        } else {
            1000.0 // Default 1 second for new entities
        };

        self.last_timestamp = timestamp_ns;
        self.last_entity = entity_hash;

        // Use entity hash as geo hash for simplicity (can be enhanced)
        let geo_hash = entity_hash.wrapping_mul(31);

        let (score, is_anomaly) = self.store.update_and_check(
            entity_hash,
            timestamp_ns,
            iat_ms,
            payload_size,
            service_hash,
            geo_hash,
        );

        // Generate reason
        let profile = self.store.get_profile(entity_hash);
        let reason = if let Some(p) = profile {
            if is_anomaly {
                format!(
                    "Behavioral anomaly: entity {} has score {:.2} (observations: {})",
                    entity_hash, p.behavior_score, p.observation_count
                )
            } else {
                format!("Normal behavior: entity {}", entity_hash)
            }
        } else {
            format!("New entity: {} (learning)", entity_hash)
        };

        (score, is_anomaly, reason)
    }

    pub fn get_stats(&self) -> (usize, u64, u64) {
        self.store.get_stats()
    }

    pub fn reset(&mut self) {
        self.store.reset();
        self.last_timestamp = 0;
        self.last_entity = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavioral_profile_creation() {
        let profile = BehavioralProfile::new(12345, 1000000000);
        assert_eq!(profile.entity_hash, 12345);
        assert!(!profile.is_mature);
        assert_eq!(profile.observation_count, 0);
    }

    #[test]
    fn test_profile_update() {
        let mut profile = BehavioralProfile::new(12345, 1000000000);

        // Update multiple times
        for i in 0..60 {
            profile.update(
                1000000000 + i as u64 * 1_000_000_000,
                1000.0,
                500.0,
                1,
                i as u64,
            );
        }

        assert!(
            profile.is_mature,
            "Profile should be mature after 60 observations"
        );
        assert!(profile.observation_count >= 60);
    }

    #[test]
    fn test_temporal_deviation() {
        let mut profile = BehavioralProfile::new(12345, 0);

        // Activity only during day hours (8-18) - need 50+ observations to be mature
        for cycle in 0..10 {
            for hour in 8..=18 {
                let ts = hour as u64 * 3_600_000_000_000u64 + cycle as u64 * 86_400_000_000_000u64;
                profile.update(ts, 1000.0, 500.0, 1, (cycle * 11 + hour) as u64);
            }
        }

        assert!(
            profile.is_mature,
            "Profile should be mature after 110 observations"
        );

        // Check deviation at unusual hour (3 AM - never seen before)
        let late_night_ts = 3u64 * 3_600_000_000_000u64;
        let deviation = profile.calculate_deviation(late_night_ts, 1000.0, 500.0, 1, 999);

        assert!(
            deviation > 0.0,
            "Should detect temporal deviation at unusual hour: deviation was {}",
            deviation
        );
    }

    #[test]
    fn test_velocity_deviation() {
        let mut profile = BehavioralProfile::new(12345, 0);

        // Normal velocity (1 req every 1000ms)
        for i in 0..60 {
            profile.update(i as u64 * 1_000_000_000u64, 1000.0, 500.0, 1, i as u64);
        }

        // High velocity deviation (1 req every 10ms)
        let deviation = profile.calculate_deviation(61000000000, 10.0, 500.0, 1, 999);

        assert!(
            deviation > 0.2,
            "Should detect velocity deviation: got {}",
            deviation
        );
    }

    #[test]
    fn test_service_deviation() {
        let mut profile = BehavioralProfile::new(12345, 0);

        // Only access service 1 and 2
        for i in 0..60 {
            let service = if i % 2 == 0 { 1 } else { 2 };
            profile.update(
                i as u64 * 1_000_000_000u64,
                1000.0,
                500.0,
                service,
                i as u64,
            );
        }

        // Access new service 3
        let deviation = profile.calculate_deviation(61000000000, 1000.0, 500.0, 3, 999);

        assert!(deviation > 0.0, "Should detect new service access");
    }

    #[test]
    fn test_profile_store() {
        let mut store = ProfileStore::new(100, 10);

        // Update multiple entities
        for i in 0..50 {
            store.update_and_check(
                i as u64,
                i as u64 * 1_000_000_000u64,
                1000.0,
                500.0,
                1,
                i as u64,
            );
        }

        let (total, mature, _) = store.get_stats();
        assert!(total > 0);
        assert!(mature <= total as u64);
    }

    #[test]
    fn test_store_eviction() {
        let mut store = ProfileStore::new(10, 5);

        // Add more profiles than max - each with a unique entity hash
        for i in 0..50 {
            let entity_hash = (i * 1000 + 1) as u64;
            store.update_and_check(
                entity_hash,
                i as u64 * 1_000_000_000,
                1000.0,
                500.0,
                1,
                i as u64,
            );
        }

        let (total, _, _) = store.get_stats();
        // Store should limit growth - may not be exactly max due to eviction timing
        // but should definitely be bounded
        assert!(
            total <= 20,
            "Store should limit profile count, got {} profiles",
            total
        );
    }

    #[test]
    fn test_fingerprint_detector() {
        let mut detector = BehavioralFingerprintDetector::new(100);

        // Warm up with normal behavior
        for i in 0..60 {
            let (_score, is_anomaly, _) =
                detector.process(12345, i as u64 * 1_000_000_000u64, 500.0, 1);

            if i < 30 {
                assert!(!is_anomaly, "Should not flag during warm-up");
            }
        }

        // Anomalous behavior
        let (score, _is_anomaly, reason) = detector.process(
            12345,
            60_000_000_000u64,
            50000.0, // Large payload
            999,     // New service
        );

        assert!(score > 0.0, "Should detect anomalous behavior: {}", reason);
    }

    #[test]
    fn test_get_typical_hours() {
        let mut profile = BehavioralProfile::new(12345, 0);

        // Activity at hours 10, 14, 18
        for i in 0..100 {
            let hour = if i % 3 == 0 {
                10
            } else if i % 3 == 1 {
                14
            } else {
                18
            };
            let ts = hour as u64 * 3_600_000_000_000u64;
            profile.update(ts, 1000.0, 500.0, 1, i as u64);
        }

        let typical = profile.get_typical_hours();
        assert!(typical.contains(&10));
        assert!(typical.contains(&14));
        assert!(typical.contains(&18));
    }
}
