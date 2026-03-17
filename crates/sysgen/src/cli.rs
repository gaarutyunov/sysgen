use clap::Parser;

use crate::commands::Command;

#[derive(Parser)]
#[command(
    name = "sysgen",
    about = "SysML v2 spec-driven code generation with AI enforcement"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.run()
}
