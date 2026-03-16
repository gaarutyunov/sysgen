/// Drives the agent generation loop until the spec is satisfied.
pub struct AgentLoop {
    agent: goose::Agent,
}

impl AgentLoop {
    pub fn new(agent: goose::Agent) -> Self {
        Self { agent }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let _ = self.agent;
        todo!("implement agent loop")
    }
}
