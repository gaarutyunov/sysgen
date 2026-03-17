#![allow(dead_code)]

mod collector;
mod report;

#[derive(clap::Parser)]
#[command(
    name = "cargo-sysgen-trace",
    about = "Standalone SysML traceability checker"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Check traceability between spec and implementation
    Check(CheckArgs),
}

#[derive(clap::Args)]
struct CheckArgs {
    /// Path to the SysML spec file
    pub spec: std::path::PathBuf,
    /// Path to the Rust source tree
    #[arg(default_value = ".")]
    pub src: std::path::PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = <Cli as clap::Parser>::parse();
    match cli.command {
        Command::Check(args) => {
            let collector = collector::TraceCollector::new(&args.src);
            let traces = collector.collect()?;
            let reporter = report::TraceReporter::new(args.spec);
            reporter.report(&traces)
        }
    }
}
