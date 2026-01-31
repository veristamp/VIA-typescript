use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EWMA {
    alpha: f64,
    mean: f64,
    variance: f64,
    initialized: bool,
}

impl EWMA {
    pub fn new(half_life: f64) -> Self {
        let alpha = 1.0 - (-std::f64::consts::LN_2 / half_life).exp();
        Self {
            alpha,
            mean: 0.0,
            variance: 0.0,
            initialized: false,
        }
    }

    pub fn update(&mut self, sample: f64) -> f64 {
        if !self.initialized {
            self.mean = sample;
            self.variance = 0.0;
            self.initialized = true;
        } else {
            let diff = sample - self.mean;
            let incr = self.alpha * diff;
            self.mean += incr;
            // Standard EWMVar update
            self.variance = (1.0 - self.alpha) * (self.variance + self.alpha * diff * diff);
        }
        self.mean
    }

    pub fn get_value(&self) -> f64 {
        self.mean
    }

    pub fn value(&self) -> f64 {
        self.mean
    }

    pub fn get_std_dev(&self) -> f64 {
        self.variance.sqrt()
    }
}
