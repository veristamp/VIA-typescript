use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct HoltWinters {
    alpha: f64,    // Level smoothing factor
    beta: f64,     // Trend smoothing factor
    gamma: f64,    // Seasonality smoothing factor
    period: usize, // Season length (e.g., 24 for hourly, 1440 for minutely)

    level: f64,
    trend: f64,
    seasonals: Vec<f64>, // Stores seasonal components

    initialized: bool,
    step: usize,
}

impl HoltWinters {
    pub fn new(alpha: f64, beta: f64, gamma: f64, period: usize) -> Self {
        Self {
            alpha,
            beta,
            gamma,
            period,
            level: 0.0,
            trend: 0.0,
            seasonals: vec![0.0; period],
            initialized: false,
            step: 0,
        }
    }

    pub fn update(&mut self, value: f64) -> (f64, f64) {
        // Returns (Expected Value, Anomaly Score [Z-Score ish])

        let season_idx = self.step % self.period;
        let last_seasonal = self.seasonals[season_idx];

        if !self.initialized {
            // Warm-up phase: Simple initialization
            if self.step == 0 {
                self.level = value;
                self.trend = 0.0;
            } else {
                // Basic initial trend estimation
                self.trend = 0.5 * self.trend + 0.5 * (value - self.level);
                self.level = value;
            }

            // Fill initial seasonality loosely
            self.seasonals[season_idx] = 0.0; // Assume flat seasonality at start

            if self.step >= self.period {
                self.initialized = true;
            }
            self.step += 1;
            return (value, 0.0);
        }

        // Prediction for NOW (before seeing actual value)
        let prediction = self.level + self.trend + last_seasonal;

        // Deviation
        let deviation = value - prediction;

        // Update Steps (Holt-Winters Additive)
        let last_level = self.level;
        let last_trend = self.trend;

        // 1. Level Update (Descriptive)
        self.level =
            self.alpha * (value - last_seasonal) + (1.0 - self.alpha) * (last_level + last_trend);

        // 2. Trend Update
        self.trend = self.beta * (self.level - last_level) + (1.0 - self.beta) * last_trend;

        // 3. Seasonality Update
        self.seasonals[season_idx] =
            self.gamma * (value - self.level) + (1.0 - self.gamma) * last_seasonal;

        self.step += 1;

        // Return Prediction and Deviation
        (prediction, deviation)
    }

    pub fn get_seasonality(&self) -> &[f64] {
        &self.seasonals
    }
}
