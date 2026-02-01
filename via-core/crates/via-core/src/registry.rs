//! Memory-Bounded Profile Registry with LRU Eviction
//!
//! This module manages the collection of AnomalyProfile instances with
//! configurable memory bounds. Uses LRU eviction to prevent unbounded growth.

use std::collections::HashMap;
use std::time::Instant;

/// Configuration for the profile registry
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Maximum number of profiles to keep
    pub max_profiles: usize,
    /// Minimum events before a profile can be evicted
    pub min_events_for_eviction: u64,
    /// Whether to track access order for LRU
    pub enable_lru: bool,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_profiles: 100_000,
            min_events_for_eviction: 10,
            enable_lru: true,
        }
    }
}

/// Metadata for a profile entry
#[derive(Debug, Clone)]
pub struct ProfileMeta {
    /// When this profile was last accessed
    pub last_access: Instant,
    /// Total events processed
    pub event_count: u64,
    /// Priority level (higher = less likely to evict)
    pub priority: u8,
    /// Creation time
    pub created_at: Instant,
}

impl Default for ProfileMeta {
    fn default() -> Self {
        Self {
            last_access: Instant::now(),
            event_count: 0,
            priority: 0,
            created_at: Instant::now(),
        }
    }
}

impl ProfileMeta {
    pub fn touch(&mut self) {
        self.last_access = Instant::now();
        self.event_count += 1;
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Calculate eviction score (lower = more likely to evict)
    pub fn eviction_score(&self) -> f64 {
        let age_seconds = self.last_access.elapsed().as_secs_f64();
        let event_factor = (self.event_count as f64).ln().max(1.0);
        let priority_factor = 1.0 + (self.priority as f64 * 0.5);

        // Higher score = keep longer
        // Recent access + more events + higher priority = higher score
        (event_factor * priority_factor) / (age_seconds + 1.0)
    }
}

/// Profile entry combining the profile and its metadata
#[derive(Debug)]
pub struct ProfileEntry<P> {
    pub profile: P,
    pub meta: ProfileMeta,
}

impl<P> ProfileEntry<P> {
    pub fn new(profile: P) -> Self {
        Self {
            profile,
            meta: ProfileMeta::default(),
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.meta.priority = priority;
        self
    }
}

/// Statistics about the registry
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    pub total_profiles: usize,
    pub total_evictions: u64,
    pub total_creations: u64,
    pub total_accesses: u64,
    pub capacity: usize,
}

/// Memory-bounded profile registry with LRU eviction
pub struct ProfileRegistry<P> {
    /// Main storage
    profiles: HashMap<u64, ProfileEntry<P>>,
    /// Configuration
    config: RegistryConfig,
    /// Statistics
    stats: RegistryStats,
    /// LRU tracking (hash -> insert order for O(1) eviction candidate)
    /// Using a simple Vec as a circular buffer
    access_order: Vec<u64>,
    access_head: usize,
}

impl<P> ProfileRegistry<P> {
    /// Create a new registry with default config
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    /// Create a new registry with custom config
    pub fn with_config(config: RegistryConfig) -> Self {
        let capacity = config.max_profiles;
        Self {
            profiles: HashMap::with_capacity(capacity),
            stats: RegistryStats {
                capacity,
                ..Default::default()
            },
            config,
            access_order: Vec::with_capacity(capacity),
            access_head: 0,
        }
    }

    /// Get profile count
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Get statistics
    pub fn stats(&self) -> &RegistryStats {
        &self.stats
    }

    /// Check if registry is at capacity
    pub fn is_full(&self) -> bool {
        self.profiles.len() >= self.config.max_profiles
    }

    /// Get a profile (if exists), updating access time
    pub fn get(&mut self, hash: u64) -> Option<&P> {
        if let Some(entry) = self.profiles.get_mut(&hash) {
            entry.meta.touch();
            self.stats.total_accesses += 1;
            Some(&entry.profile)
        } else {
            None
        }
    }

    /// Get mutable reference to profile (if exists), updating access time
    pub fn get_mut(&mut self, hash: u64) -> Option<&mut P> {
        if let Some(entry) = self.profiles.get_mut(&hash) {
            entry.meta.touch();
            self.stats.total_accesses += 1;
            Some(&mut entry.profile)
        } else {
            None
        }
    }

    /// Check if profile exists
    pub fn contains(&self, hash: u64) -> bool {
        self.profiles.contains_key(&hash)
    }

    /// Insert a new profile, evicting if necessary
    pub fn insert(&mut self, hash: u64, profile: P) -> Option<(u64, P)> {
        self.insert_with_priority(hash, profile, 0)
    }

    /// Insert with priority level
    pub fn insert_with_priority(
        &mut self,
        hash: u64,
        profile: P,
        priority: u8,
    ) -> Option<(u64, P)> {
        let mut evicted = None;

        // Evict if at capacity
        if self.is_full() && !self.profiles.contains_key(&hash) {
            evicted = self.evict_one();
        }

        let entry = ProfileEntry::new(profile).with_priority(priority);
        self.profiles.insert(hash, entry);
        self.stats.total_creations += 1;

        // Track in access order
        if self.access_order.len() < self.config.max_profiles {
            self.access_order.push(hash);
        } else {
            self.access_order[self.access_head] = hash;
            self.access_head = (self.access_head + 1) % self.access_order.len();
        }

        evicted
    }

    /// Get or create a profile
    pub fn get_or_create<F>(&mut self, hash: u64, create: F) -> &mut P
    where
        F: FnOnce() -> P,
    {
        self.get_or_create_with_priority(hash, 0, create)
    }

    /// Get or create with priority
    pub fn get_or_create_with_priority<F>(&mut self, hash: u64, priority: u8, create: F) -> &mut P
    where
        F: FnOnce() -> P,
    {
        // Check if exists
        if self.profiles.contains_key(&hash) {
            let entry = self.profiles.get_mut(&hash).unwrap();
            entry.meta.touch();
            self.stats.total_accesses += 1;
            return &mut entry.profile;
        }

        // Need to create - evict first if necessary
        if self.is_full() {
            self.evict_one();
        }

        // Create and insert
        let profile = create();
        let entry = ProfileEntry::new(profile).with_priority(priority);
        self.profiles.insert(hash, entry);
        self.stats.total_creations += 1;

        // Track access order
        if self.access_order.len() < self.config.max_profiles {
            self.access_order.push(hash);
        } else {
            self.access_order[self.access_head] = hash;
            self.access_head = (self.access_head + 1) % self.access_order.len();
        }

        &mut self.profiles.get_mut(&hash).unwrap().profile
    }

    /// Evict one profile based on LRU/score
    fn evict_one(&mut self) -> Option<(u64, P)> {
        if self.profiles.is_empty() {
            return None;
        }

        // Find best eviction candidate
        let candidate = self.find_eviction_candidate()?;

        if let Some(entry) = self.profiles.remove(&candidate) {
            self.stats.total_evictions += 1;
            Some((candidate, entry.profile))
        } else {
            None
        }
    }

    /// Find the best candidate for eviction
    fn find_eviction_candidate(&self) -> Option<u64> {
        // Simple LRU: find oldest access with low event count
        let mut best_candidate: Option<(u64, f64)> = None;

        for (&hash, entry) in &self.profiles {
            // Don't evict profiles with too few events (still learning)
            if entry.meta.event_count < self.config.min_events_for_eviction {
                continue;
            }

            let score = entry.meta.eviction_score();

            match best_candidate {
                None => best_candidate = Some((hash, score)),
                Some((_, best_score)) if score < best_score => {
                    best_candidate = Some((hash, score));
                }
                _ => {}
            }
        }

        // If no candidate met criteria, just pick the oldest
        if best_candidate.is_none() {
            best_candidate = self
                .profiles
                .iter()
                .min_by(|a, b| a.1.meta.last_access.cmp(&b.1.meta.last_access))
                .map(|(&h, e)| (h, e.meta.eviction_score()));
        }

        best_candidate.map(|(h, _)| h)
    }

    /// Remove a specific profile
    pub fn remove(&mut self, hash: u64) -> Option<P> {
        self.profiles.remove(&hash).map(|e| e.profile)
    }

    /// Clear all profiles
    pub fn clear(&mut self) {
        self.profiles.clear();
        self.access_order.clear();
        self.access_head = 0;
    }

    /// Iterate over all profiles (read-only)
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &P)> {
        self.profiles.iter().map(|(k, v)| (k, &v.profile))
    }

    /// Iterate over all entries with metadata
    pub fn iter_entries(&self) -> impl Iterator<Item = (&u64, &ProfileEntry<P>)> {
        self.profiles.iter()
    }

    /// Get all profile hashes
    pub fn hashes(&self) -> Vec<u64> {
        self.profiles.keys().copied().collect()
    }

    /// Bulk evict to reach target size
    pub fn evict_to_size(&mut self, target_size: usize) -> Vec<(u64, P)> {
        let mut evicted = Vec::new();

        while self.profiles.len() > target_size {
            if let Some(e) = self.evict_one() {
                evicted.push(e);
            } else {
                break;
            }
        }

        evicted
    }

    /// Get metadata for a profile
    pub fn get_meta(&self, hash: u64) -> Option<&ProfileMeta> {
        self.profiles.get(&hash).map(|e| &e.meta)
    }

    /// Update priority for a profile
    pub fn set_priority(&mut self, hash: u64, priority: u8) {
        if let Some(entry) = self.profiles.get_mut(&hash) {
            entry.meta.priority = priority;
        }
    }
}

impl<P> Default for ProfileRegistry<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut registry: ProfileRegistry<String> = ProfileRegistry::with_config(RegistryConfig {
            max_profiles: 10,
            min_events_for_eviction: 1,
            enable_lru: true,
        });

        // Insert
        registry.insert(1, "profile1".to_string());
        registry.insert(2, "profile2".to_string());

        assert_eq!(registry.len(), 2);
        assert!(registry.contains(1));
        assert!(registry.contains(2));

        // Get
        assert_eq!(registry.get(1), Some(&"profile1".to_string()));

        // Remove
        let removed = registry.remove(1);
        assert_eq!(removed, Some("profile1".to_string()));
        assert!(!registry.contains(1));
    }

    #[test]
    fn test_eviction() {
        let mut registry: ProfileRegistry<u32> = ProfileRegistry::with_config(RegistryConfig {
            max_profiles: 3,
            min_events_for_eviction: 0,
            enable_lru: true,
        });

        // Fill to capacity
        registry.insert(1, 100);
        registry.insert(2, 200);
        registry.insert(3, 300);

        assert_eq!(registry.len(), 3);

        // Insert one more - should evict
        registry.insert(4, 400);

        assert_eq!(registry.len(), 3);
        assert!(registry.stats().total_evictions >= 1);
    }

    #[test]
    fn test_get_or_create() {
        let mut registry: ProfileRegistry<i32> = ProfileRegistry::new();

        // First access creates
        let value = registry.get_or_create(123, || 42);
        assert_eq!(*value, 42);

        // Second access returns same
        *registry.get_or_create(123, || 999) = 100;
        assert_eq!(*registry.get(123).unwrap(), 100);

        assert_eq!(registry.stats().total_creations, 1);
    }

    #[test]
    fn test_priority_eviction() {
        let mut registry: ProfileRegistry<String> = ProfileRegistry::with_config(RegistryConfig {
            max_profiles: 3,
            min_events_for_eviction: 0,
            enable_lru: true,
        });

        // Insert with different priorities
        registry.insert_with_priority(1, "low".to_string(), 0);
        registry.insert_with_priority(2, "medium".to_string(), 5);
        registry.insert_with_priority(3, "high".to_string(), 10);

        // Touch them to ensure events
        for _ in 0..5 {
            registry.get_mut(1);
            registry.get_mut(2);
            registry.get_mut(3);
        }

        // Force eviction
        registry.insert(4, "new".to_string());

        // High priority should survive
        assert!(
            registry.contains(3),
            "High priority should survive eviction"
        );
    }
}
