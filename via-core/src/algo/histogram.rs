use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct FadingHistogram {
    decay: f64, // Decay factor (e.g., 0.999 per update)
    bins: Vec<f64>, // Weighted counts
    min_val: f64,
    max_val: f64,
    num_bins: usize,
    total_weight: f64,
}

impl FadingHistogram {
    pub fn new(num_bins: usize, min_val: f64, max_val: f64, decay: f64) -> Self {
        Self {
            decay,
            bins: vec![0.0; num_bins],
            min_val: min_val.max(0.1), // Avoid log(0)
            max_val,
            num_bins,
            total_weight: 0.0,
        }
    }

    fn get_bin_index(&self, value: f64) -> usize {
        // Log-linear binning for wide dynamic range (good for latency)
        // Log transform normalized to [0, num_bins]
        if value <= self.min_val { return 0; }
        if value >= self.max_val { return self.num_bins - 1; }
        
        let log_min = self.min_val.ln();
        let log_max = self.max_val.ln();
        let log_val = value.ln();
        
        let ratio = (log_val - log_min) / (log_max - log_min);
        ((ratio * self.num_bins as f64) as usize).min(self.num_bins - 1)
    }

    pub fn update(&mut self, value: f64) -> f64 {
        // Returns "Anomaly Score" based on probability of this bin
        
        let idx = self.get_bin_index(value);
        
        // Probability of this value occurring based on history
        let prob = if self.total_weight > 0.0 {
            self.bins[idx] / self.total_weight
        } else {
            1.0 // Assume normal if empty
        };
        
        // Decay everything
        self.total_weight *= self.decay;
        for b in &mut self.bins {
            *b *= self.decay;
        }
        
        // Add new value
        self.bins[idx] += 1.0;
        self.total_weight += 1.0;
        
        // Return Inverse Probability (Lower prob = Higher Anomaly)
        // Clip to avoid infinity
        if prob < 0.001 { 100.0 } else { 1.0 / prob }
    }
}
