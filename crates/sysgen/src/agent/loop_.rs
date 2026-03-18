use crate::agent::builder::{build_agent, AgentConfig};
use crate::agent::monitor::{make_session_config, make_user_message, process_agent_stream};
use crate::agent::prompt::{
    build_error_reprompt, build_gap_reprompt, build_initial_message, PromptContext,
};
use crate::parser::manifest::SpecManifest;
use crate::traceability::collector::collect_annotations;
use crate::traceability::report::{build_report, TraceabilityReport};
use crate::validation::cargo::{run_build, run_clippy, run_test, ValidationResult};
use anyhow::{bail, Result};
use tracing::{info, warn};

pub struct LoopConfig {
    pub agent_config: AgentConfig,
    pub project_root: std::path::PathBuf,
    pub spec_dir: std::path::PathBuf,
    pub max_iterations: u32,
}

pub struct LoopResult {
    pub success: bool,
    pub iterations: u32,
    pub final_report: Option<TraceabilityReport>,
}

/// Run the generation loop until all validation gates pass or a circuit breaker trips.
///
/// The loop drives the Goose agent through repeated generate → validate cycles:
///
/// ```text
/// GENERATE → BUILD → CLIPPY → TEST → TRACEABILITY → SUCCESS
///              ↑        ↑       ↑          ↑
///              └────────┴───────┴──────────┘  (on failure, re-prompt and restart from BUILD)
/// ```
///
/// # Circuit breakers
///
/// - **Max iterations**: aborts after `config.max_iterations` cycles (default 20).
/// - **Stuck detection**: aborts if the same error fingerprint appears 3 consecutive times.
/// - **Regression detection**: aborts if error count increases 3 consecutive times.
pub async fn run_generation_loop(
    manifest: &SpecManifest,
    config: &LoopConfig,
) -> Result<LoopResult> {
    let agent = build_agent(&config.agent_config).await?;
    let session_config = make_session_config(&config.agent_config.session_id);

    let ctx = PromptContext {
        manifest: manifest.clone(),
        project_root: config.project_root.clone(),
        target_language: "rust".to_string(),
    };
    let initial_message_text = build_initial_message(&ctx)?;

    // The current user message to send on the next GENERATE step.
    let mut current_user_message = make_user_message(&initial_message_text);

    // Circuit breaker state
    let mut iteration = 0u32;
    let mut consecutive_same_errors = 0u32;
    let mut consecutive_regressions = 0u32;
    let mut last_error_fingerprint = String::new();
    let mut last_error_count = usize::MAX;

    loop {
        if iteration >= config.max_iterations {
            bail!(
                "Max iterations ({}) reached without success. \
                 Use --max-iterations to increase the limit.",
                config.max_iterations
            );
        }
        iteration += 1;
        info!(
            "=== Generation iteration {}/{} ===",
            iteration, config.max_iterations
        );

        // ── GENERATE ─────────────────────────────────────────
        let run_result = process_agent_stream(
            &agent,
            current_user_message,
            session_config.clone(),
            &config.spec_dir,
        )
        .await?;

        if run_result.spec_write_attempted {
            warn!("Agent attempted to write spec/ — was blocked. Continuing.");
        }

        // ── BUILD ─────────────────────────────────────────────
        info!("Stage: cargo build");
        match run_build(&config.project_root).await? {
            ValidationResult::Failure {
                output,
                error_count,
            } => {
                let fingerprint = fingerprint_errors(&output);
                check_stuck_circuit_breaker(
                    &fingerprint,
                    &last_error_fingerprint,
                    &mut consecutive_same_errors,
                    error_count,
                    last_error_count,
                    &mut consecutive_regressions,
                )?;
                last_error_fingerprint = fingerprint;
                last_error_count = error_count;

                let reprompt = build_error_reprompt("cargo build", &output, iteration);
                current_user_message = make_user_message(&reprompt);
                continue;
            }
            ValidationResult::Success => {
                info!("✓ cargo build passed");
                reset_circuit_breaker_state(
                    &mut last_error_fingerprint,
                    &mut last_error_count,
                    &mut consecutive_same_errors,
                    &mut consecutive_regressions,
                );
            }
        }

        // ── CLIPPY ────────────────────────────────────────────
        info!("Stage: cargo clippy");
        match run_clippy(&config.project_root).await? {
            ValidationResult::Failure {
                output,
                error_count,
            } => {
                let fingerprint = fingerprint_errors(&output);
                check_stuck_circuit_breaker(
                    &fingerprint,
                    &last_error_fingerprint,
                    &mut consecutive_same_errors,
                    error_count,
                    last_error_count,
                    &mut consecutive_regressions,
                )?;
                last_error_fingerprint = fingerprint;
                last_error_count = error_count;

                let reprompt = build_error_reprompt("cargo clippy", &output, iteration);
                current_user_message = make_user_message(&reprompt);
                continue;
            }
            ValidationResult::Success => {
                info!("✓ cargo clippy passed");
                reset_circuit_breaker_state(
                    &mut last_error_fingerprint,
                    &mut last_error_count,
                    &mut consecutive_same_errors,
                    &mut consecutive_regressions,
                );
            }
        }

        // ── TEST ──────────────────────────────────────────────
        info!("Stage: cargo test");
        match run_test(&config.project_root).await? {
            ValidationResult::Failure {
                output,
                error_count,
            } => {
                let fingerprint = fingerprint_errors(&output);
                check_stuck_circuit_breaker(
                    &fingerprint,
                    &last_error_fingerprint,
                    &mut consecutive_same_errors,
                    error_count,
                    last_error_count,
                    &mut consecutive_regressions,
                )?;
                last_error_fingerprint = fingerprint;
                last_error_count = error_count;

                let reprompt = build_error_reprompt("cargo test", &output, iteration);
                current_user_message = make_user_message(&reprompt);
                continue;
            }
            ValidationResult::Success => {
                info!("✓ cargo test passed");
                reset_circuit_breaker_state(
                    &mut last_error_fingerprint,
                    &mut last_error_count,
                    &mut consecutive_same_errors,
                    &mut consecutive_regressions,
                );
            }
        }

        // ── TRACEABILITY ──────────────────────────────────────
        info!("Stage: traceability check");
        let annotations = collect_annotations(&config.project_root.join("src"))?;
        let report = build_report(manifest, &annotations);

        if !report.is_complete() {
            warn!(
                "Traceability gaps: {}/{} covered",
                report.fully_covered, report.total_requirements
            );
            let gap_msg = report.format_gap_message();
            let reprompt = build_gap_reprompt(&gap_msg, iteration, config.max_iterations);
            current_user_message = make_user_message(&reprompt);
            continue;
        }

        // ── SUCCESS ───────────────────────────────────────────
        info!(
            "✅ All gates passed! {}/{} requirements fully covered.",
            report.fully_covered, report.total_requirements
        );
        return Ok(LoopResult {
            success: true,
            iterations: iteration,
            final_report: Some(report),
        });
    }
}

/// Compute a short hex fingerprint of error output for stuck-detection.
pub fn fingerprint_errors(output: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(output.as_bytes());
    hex::encode(&hash[..8])
}

/// Check and update stuck/regression circuit breaker state.
///
/// Returns `Err` if a circuit breaker threshold is exceeded.
pub fn check_stuck_circuit_breaker(
    fingerprint: &str,
    last_fingerprint: &str,
    consecutive_same: &mut u32,
    error_count: usize,
    last_error_count: usize,
    consecutive_regressions: &mut u32,
) -> Result<()> {
    if fingerprint == last_fingerprint {
        *consecutive_same += 1;
        if *consecutive_same >= 3 {
            bail!(
                "STUCK: The same errors have appeared {} consecutive times. \
                 The agent is not making progress. Aborting.",
                consecutive_same
            );
        }
    } else {
        *consecutive_same = 0;
    }

    if error_count > last_error_count && last_error_count != usize::MAX {
        *consecutive_regressions += 1;
        if *consecutive_regressions >= 3 {
            bail!(
                "REGRESSION: Error count increased {} consecutive times ({} → {}). \
                 The agent is making things worse. Aborting.",
                consecutive_regressions,
                last_error_count,
                error_count
            );
        }
    } else {
        *consecutive_regressions = 0;
    }

    Ok(())
}

/// Reset circuit breaker state after a stage passes successfully.
fn reset_circuit_breaker_state(
    last_fingerprint: &mut String,
    last_error_count: &mut usize,
    consecutive_same: &mut u32,
    consecutive_regressions: &mut u32,
) {
    last_fingerprint.clear();
    *last_error_count = usize::MAX;
    *consecutive_same = 0;
    *consecutive_regressions = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── fingerprint_errors ───────────────────────────────────

    #[test]
    fn fingerprint_errors_is_16_hex_chars() {
        let fp = fingerprint_errors("error[E0001]: something");
        assert_eq!(fp.len(), 16, "8 bytes = 16 hex chars");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_errors_same_input_same_output() {
        let a = fingerprint_errors("error: foo");
        let b = fingerprint_errors("error: foo");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_errors_different_input_different_output() {
        let a = fingerprint_errors("error: foo");
        let b = fingerprint_errors("error: bar");
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_errors_empty_input() {
        let fp = fingerprint_errors("");
        assert_eq!(fp.len(), 16);
    }

    // ── check_stuck_circuit_breaker ──────────────────────────

    #[test]
    fn stuck_detection_triggers_after_three_consecutive_identical() {
        let fp = "aabbccdd11223344";
        let mut last = String::new();
        let mut same = 0u32;
        let mut regressions = 0u32;

        // First occurrence: no error yet
        check_stuck_circuit_breaker(fp, &last, &mut same, 5, usize::MAX, &mut regressions).unwrap();
        last = fp.to_string();
        assert_eq!(same, 0); // fingerprint changed from "" to fp

        // Second consecutive identical
        check_stuck_circuit_breaker(fp, &last, &mut same, 5, 5, &mut regressions).unwrap();
        assert_eq!(same, 1);

        // Third consecutive identical
        check_stuck_circuit_breaker(fp, &last, &mut same, 5, 5, &mut regressions).unwrap();
        assert_eq!(same, 2);

        // Fourth — should abort
        last = fp.to_string();
        same = 3; // simulate already at threshold
        let result = check_stuck_circuit_breaker(fp, &last, &mut same, 5, 5, &mut regressions);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("STUCK"));
    }

    #[test]
    fn stuck_detection_resets_when_fingerprint_changes() {
        let fp1 = "aaaa000011112222";
        let fp2 = "bbbb000011112222";
        let last = fp1.to_string();
        let mut same = 2u32;
        let mut regressions = 0u32;

        // Different fingerprint — should reset counter
        check_stuck_circuit_breaker(fp2, &last, &mut same, 3, 5, &mut regressions).unwrap();
        assert_eq!(same, 0);
    }

    #[test]
    fn regression_detection_triggers_after_three_consecutive_increases() {
        let fp = "aabbccdd11223344";
        let mut last = String::new();
        let mut same = 0u32;
        let mut regressions = 0u32;

        // 1 → 2: increase
        check_stuck_circuit_breaker(fp, &last, &mut same, 2, 1, &mut regressions).unwrap();
        assert_eq!(regressions, 1);
        last = fp.to_string();

        // 2 → 3: increase
        check_stuck_circuit_breaker(fp, &last, &mut same, 3, 2, &mut regressions).unwrap();
        assert_eq!(regressions, 2);

        // 3 → 5: increase — should abort
        let result = check_stuck_circuit_breaker(fp, &last, &mut same, 5, 3, &mut regressions);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("REGRESSION"));
    }

    #[test]
    fn regression_detection_resets_when_count_does_not_increase() {
        let fp = "aabbccdd11223344";
        let last = String::new();
        let mut same = 0u32;
        let mut regressions = 2u32;

        // Count decreases — should reset regression counter
        check_stuck_circuit_breaker(fp, &last, &mut same, 2, 5, &mut regressions).unwrap();
        assert_eq!(regressions, 0);
    }

    #[test]
    fn regression_detection_skips_when_last_count_is_max() {
        let fp = "aabbccdd11223344";
        let last = String::new();
        let mut same = 0u32;
        let mut regressions = 0u32;

        // First failure — last_error_count is usize::MAX, should NOT count as regression
        check_stuck_circuit_breaker(fp, &last, &mut same, 10, usize::MAX, &mut regressions)
            .unwrap();
        assert_eq!(regressions, 0);
    }

    // ── reset_circuit_breaker_state ──────────────────────────

    #[test]
    fn reset_clears_all_state() {
        let mut fp = "some_fingerprint".to_string();
        let mut count = 5usize;
        let mut same = 2u32;
        let mut regressions = 1u32;

        reset_circuit_breaker_state(&mut fp, &mut count, &mut same, &mut regressions);

        assert!(fp.is_empty());
        assert_eq!(count, usize::MAX);
        assert_eq!(same, 0);
        assert_eq!(regressions, 0);
    }
}
