/// Structures compiler/clippy output into actionable feedback for the agent.
pub struct FeedbackCollector;

impl FeedbackCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self) -> anyhow::Result<Vec<String>> {
        todo!("implement feedback collector")
    }
}

impl Default for FeedbackCollector {
    fn default() -> Self {
        Self::new()
    }
}
