pub mod traffic;
pub mod performance;
pub mod security;

use crate::simulation::types::LogRecord;

pub trait Scenario {
    fn name(&self) -> &str;
    fn tick(&mut self, current_time_ns: u64, delta_ns: u64) -> Vec<LogRecord>;
}
