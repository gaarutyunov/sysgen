/// Monitors agent output and surfaces structured feedback.
pub struct AgentMonitor;

impl AgentMonitor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentMonitor {
    fn default() -> Self {
        Self::new()
    }
}
