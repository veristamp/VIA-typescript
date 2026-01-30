pub struct CUSUM {
    target: f64,
    slack: f64,
    threshold: f64,
    c_pos: f64,
    c_neg: f64,
    pub alarm: bool,
    pub alarm_type: i8, // 0=none, 1=high, -1=low
}

impl CUSUM {
    pub fn new(target: f64, slack: f64, threshold: f64) -> Self {
        Self {
            target,
            slack,
            threshold,
            c_pos: 0.0,
            c_neg: 0.0,
            alarm: false,
            alarm_type: 0,
        }
    }

    pub fn update(&mut self, sample: f64) -> bool {
        self.alarm = false;
        self.alarm_type = 0;

        let deviation = sample - self.target;

        self.c_pos = (self.c_pos + deviation - self.slack).max(0.0);
        self.c_neg = (self.c_neg - deviation - self.slack).max(0.0);

        if self.c_pos > self.threshold {
            self.alarm = true;
            self.alarm_type = 1;
            self.c_pos = 0.0; // Reset after alarm
            return true;
        }

        if self.c_neg > self.threshold {
            self.alarm = true;
            self.alarm_type = -1;
            self.c_neg = 0.0;
            return true;
        }

        false
    }
}
