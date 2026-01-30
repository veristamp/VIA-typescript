pub mod performance;
pub mod security;
pub mod traffic;

use crate::simulation::types::LogRecord;

pub trait Scenario {
    fn name(&self) -> &str;
    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord>;
}
