#[derive(clap::Args)]
pub struct WatchCommand {
    /// Path to the SysML spec file
    pub spec: std::path::PathBuf,
}

impl WatchCommand {
    pub fn run(self) -> anyhow::Result<()> {
        todo!("implement watch command")
    }
}
