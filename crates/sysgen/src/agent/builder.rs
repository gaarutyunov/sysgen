/// Builds a Goose agent configured for SysGen code generation.
pub struct AgentBuilder {
    inner: goose::AgentBuilder,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            inner: goose::AgentBuilder::new(),
        }
    }

    pub fn build(self) -> goose::Agent {
        self.inner.build()
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
