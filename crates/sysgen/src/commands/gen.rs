#[derive(clap::Args)]
pub struct GenCommand {
    /// Directory containing .sysml spec files (default: ./spec)
    #[arg(long, default_value = "spec")]
    spec_dir: std::path::PathBuf,

    /// Root of the Rust project to generate code into (default: current dir)
    #[arg(long, default_value = ".")]
    project_root: std::path::PathBuf,

    /// LLM provider: anthropic, openai, google, ollama
    #[arg(long, default_value = "anthropic", env = "SYSGEN_PROVIDER")]
    provider: String,

    /// LLM model identifier
    #[arg(long, env = "SYSGEN_MODEL")]
    model: Option<String>,

    /// Maximum agent iterations before aborting
    #[arg(long, default_value_t = 20)]
    max_iterations: u32,

    /// Print the generated prompt and exit (dry-run for debugging)
    #[arg(long)]
    dry_run: bool,

    /// Output traceability report as JSON to this file after completion
    #[arg(long)]
    report_output: Option<std::path::PathBuf>,
}

impl GenCommand {
    pub fn run(self) -> anyhow::Result<()> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(run_gen(self))
    }
}

async fn run_gen(args: GenCommand) -> anyhow::Result<()> {
    use crate::agent::builder::{assert_provider_env_var, AgentConfig};
    use crate::agent::loop_::{run_generation_loop, LoopConfig};
    use crate::freeze::guard::{write_gooseignore, SpecFreezeGuard};
    use crate::parser::workspace::load_spec_manifest;

    // 1. Validate environment
    assert_provider_env_var(&args.provider)?;
    let model = args.model.unwrap_or_else(|| default_model(&args.provider));

    // 2. Parse spec files
    println!("📖 Parsing SysML v2 spec files from {:?}...", args.spec_dir);
    let manifest = load_spec_manifest(&args.spec_dir)?;
    println!("   Found {} requirements:", manifest.requirements.len());
    let mut sorted_reqs: Vec<_> = manifest.requirements.values().collect();
    sorted_reqs.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
    for req in sorted_reqs {
        println!(
            "   • {} ({})",
            req.qualified_name,
            req.short_name.as_deref().unwrap_or("—")
        );
    }
    println!();

    // 3. Dry run — print prompt and exit
    if args.dry_run {
        let ctx = crate::agent::prompt::PromptContext {
            manifest: manifest.clone(),
            project_root: args.project_root.clone(),
            target_language: "rust".to_string(),
        };
        let prompt = crate::agent::prompt::build_initial_message(&ctx)?;
        println!("=== DRY RUN: Generated prompt ===\n");
        println!("{}", prompt);
        return Ok(());
    }

    // 4. Write .gooseignore
    write_gooseignore(&args.project_root, &args.spec_dir)?;

    // 5. Activate spec freeze
    println!("🔒 Activating spec file freeze...");
    let mut freeze_guard = SpecFreezeGuard::new(&args.spec_dir)?;
    freeze_guard.activate()?;

    // 6. Run the agent loop
    let loop_result = {
        let loop_config = LoopConfig {
            agent_config: AgentConfig {
                provider: args.provider.clone(),
                model,
                working_dir: args.project_root.clone(),
                session_id: format!("sysgen-{}", chrono::Utc::now().timestamp()),
            },
            project_root: args.project_root.clone(),
            spec_dir: args.spec_dir.clone(),
            max_iterations: args.max_iterations,
        };

        println!(
            "🚀 Starting generation loop (max {} iterations)...",
            args.max_iterations
        );
        run_generation_loop(&manifest, &loop_config).await
    };

    // 7. Verify freeze integrity after loop
    freeze_guard.verify_integrity()?;

    // 8. Deactivate freeze
    freeze_guard.deactivate()?;

    // 9. Handle result
    match loop_result {
        Ok(result) => {
            println!();
            println!(
                "✅ Generation complete in {} iteration(s)!",
                result.iterations
            );
            if let Some(report) = &result.final_report {
                println!(
                    "📊 Traceability: {}/{} requirements fully covered ({:.1}%)",
                    report.fully_covered, report.total_requirements, report.coverage_percent
                );
                if let Some(output_path) = args.report_output {
                    let json = serde_json::to_string_pretty(report)?;
                    std::fs::write(&output_path, json)?;
                    println!("   Report written to {:?}", output_path);
                }
            }
        }
        Err(e) => {
            eprintln!();
            eprintln!("❌ Generation failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn default_model(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514".to_string(),
        "openai" => "gpt-4o".to_string(),
        "google" => "gemini-2.5-pro".to_string(),
        "ollama" => "llama3.2".to_string(),
        _ => "claude-sonnet-4-20250514".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_anthropic() {
        assert_eq!(default_model("anthropic"), "claude-sonnet-4-20250514");
    }

    #[test]
    fn default_model_openai() {
        assert_eq!(default_model("openai"), "gpt-4o");
    }

    #[test]
    fn default_model_google() {
        assert_eq!(default_model("google"), "gemini-2.5-pro");
    }

    #[test]
    fn default_model_ollama() {
        assert_eq!(default_model("ollama"), "llama3.2");
    }

    #[test]
    fn default_model_unknown_falls_back_to_claude() {
        assert_eq!(default_model("unknown"), "claude-sonnet-4-20250514");
    }
}
