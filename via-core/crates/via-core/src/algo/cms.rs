use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3;

/// Count-Min Sketch for memory-efficient frequency estimation
///
/// Provides a fixed-size data structure to estimate frequencies of
/// elements in a stream with a small probability of overestimation.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CountMinSketch {
    width: usize,
    depth: usize,
    table: Vec<u32>,
}

impl CountMinSketch {
    pub fn new(width: usize, depth: usize) -> Self {
        Self {
            width,
            depth,
            table: vec![0; width * depth],
        }
    }

    /// Create with default dimensions (optimised for memory/accuracy trade-off)
    pub fn default_sketch() -> Self {
        Self::new(64, 4) // 256 * 4 bytes = 1KB
    }

    pub fn increment(&mut self, hash: u64) {
        for d in 0..self.depth {
            // Use different seeds for each row
            let h = xxh3::xxh3_64_with_seed(&hash.to_le_bytes(), d as u64);
            let w = (h as usize) % self.width;
            self.table[d * self.width + w] = self.table[d * self.width + w].saturating_add(1);
        }
    }

    pub fn estimate(&self, hash: u64) -> u32 {
        let mut min_val = u32::MAX;
        for d in 0..self.depth {
            let h = xxh3::xxh3_64_with_seed(&hash.to_le_bytes(), d as u64);
            let w = (h as usize) % self.width;
            let val = self.table[d * self.width + w];
            if val < min_val {
                min_val = val;
            }
        }
        min_val
    }

    pub fn contains(&self, hash: u64) -> bool {
        self.estimate(hash) > 0
    }

    pub fn clear(&mut self) {
        for val in &mut self.table {
            *val = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cms_basic() {
        let mut cms = CountMinSketch::new(10, 4);
        cms.increment(12345);
        cms.increment(12345);
        cms.increment(67890);

        assert_eq!(cms.estimate(12345), 2);
        assert_eq!(cms.estimate(67890), 1);
        assert_eq!(cms.estimate(11111), 0);
    }

    #[test]
    fn test_cms_collision() {
        // Small width to force collisions
        let mut cms = CountMinSketch::new(2, 2);
        for i in 0..100 {
            cms.increment(i);
        }
        // Estimates should be >= actual (which is 1 for each)
        assert!(cms.estimate(1) >= 1);
    }
}
