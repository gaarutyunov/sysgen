#[derive(clap::Args)]
pub struct GenCommand {
    /// Path to the SysML spec file
    pub spec: std::path::PathBuf,
}

impl GenCommand {
    pub fn run(self) -> anyhow::Result<()> {
        todo!("implement gen command")
    }
}
