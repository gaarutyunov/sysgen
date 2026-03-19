use anyhow::Result;
use std::path::Path;
use tokio::process::Command;

/// Outcome of a single cargo validation stage.
pub enum ValidationResult {
    /// The command exited successfully.
    Success,
    /// The command failed; `output` is the combined stdout+stderr and
    /// `error_count` is the number of distinct error lines detected.
    Failure { output: String, error_count: usize },
}

/// Run `cargo build` in `project_root`.
pub async fn run_build(project_root: &Path) -> Result<ValidationResult> {
    run_cargo(project_root, &["build"]).await
}

/// Run `cargo clippy -- -D warnings` in `project_root`.
pub async fn run_clippy(project_root: &Path) -> Result<ValidationResult> {
    run_cargo(project_root, &["clippy", "--", "-D", "warnings"]).await
}

/// Run `cargo test` in `project_root`.
pub async fn run_test(project_root: &Path) -> Result<ValidationResult> {
    run_cargo(project_root, &["test"]).await
}

async fn run_cargo(project_root: &Path, args: &[&str]) -> Result<ValidationResult> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(project_root)
        .output()
        .await?;

    if output.status.success() {
        return Ok(ValidationResult::Success);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let combined = format!("{stdout}\n{stderr}").trim().to_string();
    let error_count = count_errors(&combined);

    Ok(ValidationResult::Failure {
        output: combined,
        error_count,
    })
}

/// Count distinct error occurrences in cargo/compiler output.
fn count_errors(output: &str) -> usize {
    output
        .lines()
        .filter(|line| {
            line.contains("error[")
                || (line.starts_with("error:") || line.contains(" error:"))
                || line.contains("FAILED")
                || line.contains("test result: FAILED")
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_errors_detects_compiler_error_codes() {
        let output = "error[E0001]: some message\nerror[E0002]: another\n";
        assert_eq!(count_errors(output), 2);
    }

    #[test]
    fn count_errors_detects_test_failures() {
        let output = "test foo ... FAILED\ntest result: FAILED. 1 failed; 0 passed";
        assert_eq!(count_errors(output), 2);
    }

    #[test]
    fn count_errors_ignores_warnings() {
        let output = "warning: unused variable\nnote: something";
        assert_eq!(count_errors(output), 0);
    }

    #[test]
    fn count_errors_returns_zero_for_empty() {
        assert_eq!(count_errors(""), 0);
    }
}
