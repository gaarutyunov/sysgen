/// Stub implementation of the goose agent framework.
/// Replace with the real git dependency once the commit SHA is known.

pub struct Agent;

#[derive(Default)]
pub struct AgentBuilder;

impl AgentBuilder {
    pub fn new() -> Self {
        AgentBuilder
    }

    pub fn build(self) -> Agent {
        Agent
    }
}
