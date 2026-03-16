#[derive(clap::Args)]
pub struct InitCommand {
    /// Path to initialise (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,
}

impl InitCommand {
    pub fn run(self) -> anyhow::Result<()> {
        todo!("implement init command")
    }
}
