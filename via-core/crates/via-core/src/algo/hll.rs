use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HyperLogLog {
    registers: Vec<u8>,
    p: u8,
    m: usize,
    alpha_mm: f64,
}

impl HyperLogLog {
    pub fn new(precision: u8) -> Self {
        let p = precision.clamp(4, 16);
        let m = 1 << p;
        let alpha = match p {
            4 => 0.673,
            5 => 0.697,
            6 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m as f64),
        };
        let alpha_mm = alpha * (m as f64) * (m as f64);

        Self {
            registers: vec![0; m],
            p,
            m,
            alpha_mm,
        }
    }

    pub fn add(&mut self, value: &str) {
        let hash = xxh3::xxh3_64(value.as_bytes());
        self.add_hash(hash);
    }

    pub fn add_hash(&mut self, hash: u64) {
        let idx = (hash >> (64 - self.p)) as usize;
        let w = hash << self.p; // Remaining bits
        let lz = (w.leading_zeros() as u8) + 1;

        if lz > self.registers[idx] {
            self.registers[idx] = lz;
        }
    }

    pub fn count(&self) -> f64 {
        let mut raw_sum = 0.0;
        let mut zeros = 0;

        for &reg in &self.registers {
            raw_sum += 1.0 / (1u64 << reg) as f64;
            if reg == 0 {
                zeros += 1;
            }
        }

        let mut estimate = self.alpha_mm / raw_sum;

        if estimate <= 2.5 * (self.m as f64) {
            if zeros > 0 {
                estimate = (self.m as f64) * ((self.m as f64) / (zeros as f64)).ln();
            }
        }

        estimate
    }
}
