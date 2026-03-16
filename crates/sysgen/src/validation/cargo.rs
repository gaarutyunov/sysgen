/// Runs the cargo build/clippy/test validation loop.
pub struct CargoValidator;

impl CargoValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self) -> anyhow::Result<()> {
        todo!("implement cargo validator")
    }
}

impl Default for CargoValidator {
    fn default() -> Self {
        Self::new()
    }
}
