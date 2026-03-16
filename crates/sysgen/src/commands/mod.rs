pub mod check;
pub mod gen;
pub mod init;
pub mod watch;

#[derive(clap::Subcommand)]
pub enum Command {
    Init(init::InitCommand),
    Gen(gen::GenCommand),
    Check(check::CheckCommand),
    Watch(watch::WatchCommand),
}

impl Command {
    pub fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Init(cmd) => cmd.run(),
            Command::Gen(cmd) => cmd.run(),
            Command::Check(cmd) => cmd.run(),
            Command::Watch(cmd) => cmd.run(),
        }
    }
}
