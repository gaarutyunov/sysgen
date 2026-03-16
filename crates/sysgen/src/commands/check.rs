#[derive(clap::Args)]
pub struct CheckCommand {
    /// Path to the SysML spec file
    pub spec: std::path::PathBuf,
}

impl CheckCommand {
    pub fn run(self) -> anyhow::Result<()> {
        todo!("implement check command")
    }
}
