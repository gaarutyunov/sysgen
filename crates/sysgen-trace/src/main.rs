use sysgen::traceability::collector::collect_annotations;
use sysgen::traceability::report::{build_report, print_text_report};

/// cargo-sysgen-trace: standalone traceability checker
/// Run as: cargo sysgen-trace --spec-dir spec/ --src-dir src/
#[derive(clap::Parser)]
#[command(name = "cargo-sysgen-trace")]
struct Args {
    #[arg(long, default_value = "spec")]
    spec_dir: std::path::PathBuf,

    #[arg(long, default_value = "src")]
    src_dir: std::path::PathBuf,

    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    format: String,

    /// Exit with code 1 if any gaps found
    #[arg(long, default_value_t = true)]
    fail_on_gaps: bool,
}

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::Parser>::parse();
    let manifest = sysgen::parser::workspace::load_spec_manifest(&args.spec_dir)?;
    let annotations = collect_annotations(&args.src_dir)?;
    let report = build_report(&manifest, &annotations);

    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print_text_report(&report),
    }

    if args.fail_on_gaps && !report.is_complete() {
        std::process::exit(1);
    }
    Ok(())
}
