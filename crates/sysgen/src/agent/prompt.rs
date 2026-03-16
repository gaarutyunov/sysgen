/// Builds prompts for the Goose agent from SysML spec elements.
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self) -> String {
        todo!("implement prompt builder")
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
