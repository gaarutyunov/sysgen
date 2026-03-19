use anyhow::Result;
use cargo_metadata::diagnostic::DiagnosticLevel;
use cargo_metadata::Message;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Outcome of a single cargo validation stage.
pub enum ValidationResult {
    /// The command exited successfully.
    Success,
    /// The command failed; `output` is formatted error text for the LLM and
    /// `error_count` is the number of distinct errors.
    Failure { output: String, error_count: usize },
}

/// Run `cargo build --message-format=json` in `project_root`.
pub async fn run_build(project_root: &Path) -> Result<ValidationResult> {
    run_cargo_with_json(project_root, &["build", "--message-format=json"]).await
}

/// Run `cargo clippy --message-format=json -- -D warnings` in `project_root`.
pub async fn run_clippy(project_root: &Path) -> Result<ValidationResult> {
    run_cargo_with_json(
        project_root,
        &["clippy", "--message-format=json", "--", "-D", "warnings"],
    )
    .await
}

/// Run `cargo test` in `project_root`.
pub async fn run_test(project_root: &Path) -> Result<ValidationResult> {
    let output = Command::new("cargo")
        .args(["test", "--", "--test-output", "immediate"])
        .current_dir(project_root)
        .output()
        .await?;

    if output.status.success() {
        return Ok(ValidationResult::Success);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let combined = format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr);
    let error_count = count_test_failures(&combined);

    Ok(ValidationResult::Failure {
        output: format_test_output_for_llm(&combined),
        error_count,
    })
}

async fn run_cargo_with_json(project_root: &Path, args: &[&str]) -> Result<ValidationResult> {
    let mut child = Command::new("cargo")
        .args(args)
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout not captured");
    let mut lines = BufReader::new(stdout).lines();

    let mut errors: Vec<FormattedError> = Vec::new();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Message>(&line) {
            Ok(Message::CompilerMessage(msg)) => {
                let diag = &msg.message;
                if matches!(
                    diag.level,
                    DiagnosticLevel::Error | DiagnosticLevel::Warning
                ) {
                    errors.push(FormattedError {
                        message: diag.message.clone(),
                        code: diag.code.as_ref().map(|c| c.code.clone()),
                        rendered: diag.rendered.clone(),
                        spans: diag
                            .spans
                            .iter()
                            .map(|s| SpanInfo {
                                file: s.file_name.clone(),
                                line_start: s.line_start,
                                line_end: s.line_end,
                                label: s.label.clone(),
                            })
                            .collect(),
                        suggestions: diag
                            .children
                            .iter()
                            .filter_map(|c| c.rendered.clone())
                            .collect(),
                    });
                }
            }
            Ok(Message::BuildFinished(bf)) => {
                if bf.success && errors.is_empty() {
                    let _ = child.wait().await;
                    return Ok(ValidationResult::Success);
                }
            }
            _ => {}
        }
    }

    let status = child.wait().await?;
    if status.success() && errors.is_empty() {
        return Ok(ValidationResult::Success);
    }

    let output = format_errors_for_llm(&errors);
    Ok(ValidationResult::Failure {
        output,
        error_count: errors.len(),
    })
}

#[derive(Debug)]
struct FormattedError {
    message: String,
    code: Option<String>,
    rendered: Option<String>,
    spans: Vec<SpanInfo>,
    suggestions: Vec<String>,
}

#[derive(Debug)]
struct SpanInfo {
    file: String,
    line_start: usize,
    line_end: usize,
    label: Option<String>,
}

fn format_errors_for_llm(errors: &[FormattedError]) -> String {
    let mut out = String::new();
    for (i, err) in errors.iter().enumerate() {
        out.push_str(&format!("Error {}/{}:\n", i + 1, errors.len()));
        if let Some(code) = &err.code {
            out.push_str(&format!("  Code: {}\n", code));
        }
        out.push_str(&format!("  Message: {}\n", err.message));
        for span in &err.spans {
            out.push_str(&format!(
                "  Location: {}:{}:{}\n",
                span.file, span.line_start, span.line_end
            ));
            if let Some(label) = &span.label {
                out.push_str(&format!("  Note: {}\n", label));
            }
        }
        if let Some(rendered) = &err.rendered {
            let clean = strip_ansi_codes(rendered);
            out.push_str(&format!("  Detail:\n{}\n", indent(&clean, "    ")));
        }
        for suggestion in &err.suggestions {
            out.push_str(&format!("  Suggestion: {}\n", suggestion));
        }
        out.push('\n');
    }
    out
}

fn count_test_failures(output: &str) -> usize {
    output
        .lines()
        .filter(|l| l.contains("FAILED") || l.contains("test result: FAILED"))
        .count()
}

fn format_test_output_for_llm(output: &str) -> String {
    let mut result = String::new();
    let mut in_failure = false;

    for line in output.lines() {
        if line.contains("---- ") && line.contains(" stdout ----") {
            in_failure = true;
        }
        if in_failure {
            result.push_str(line);
            result.push('\n');
        }
        if line.starts_with("test ") && line.contains(" ... FAILED") {
            result.push_str(line);
            result.push('\n');
        }
        if result.len() > 8000 {
            result.push_str("\n[... truncated for brevity ...]\n");
            break;
        }
    }

    result
}

fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

fn indent(s: &str, prefix: &str) -> String {
    s.lines()
        .map(|l| format!("{}{}", prefix, l))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_test_failures_detects_failed_tests() {
        let output = "test foo ... FAILED\ntest result: FAILED. 1 failed; 0 passed";
        assert_eq!(count_test_failures(output), 2);
    }

    #[test]
    fn count_test_failures_ignores_passing() {
        let output = "test foo ... ok\ntest result: ok. 1 passed; 0 failed";
        assert_eq!(count_test_failures(output), 0);
    }

    #[test]
    fn strip_ansi_codes_removes_escape_sequences() {
        let colored = "\x1b[31mError\x1b[0m: something";
        assert_eq!(strip_ansi_codes(colored), "Error: something");
    }

    #[test]
    fn strip_ansi_codes_leaves_plain_text() {
        let plain = "no colors here";
        assert_eq!(strip_ansi_codes(plain), plain);
    }

    #[test]
    fn indent_adds_prefix_to_each_line() {
        let input = "line1\nline2";
        assert_eq!(indent(input, "  "), "  line1\n  line2");
    }

    #[test]
    fn format_errors_for_llm_includes_all_fields() {
        let errors = vec![FormattedError {
            message: "type mismatch".to_string(),
            code: Some("E0308".to_string()),
            rendered: Some("rendered output".to_string()),
            spans: vec![SpanInfo {
                file: "src/main.rs".to_string(),
                line_start: 10,
                line_end: 10,
                label: Some("expected i32".to_string()),
            }],
            suggestions: vec!["try this fix".to_string()],
        }];

        let output = format_errors_for_llm(&errors);
        assert!(output.contains("Error 1/1:"));
        assert!(output.contains("Code: E0308"));
        assert!(output.contains("Message: type mismatch"));
        assert!(output.contains("Location: src/main.rs:10:10"));
        assert!(output.contains("Note: expected i32"));
        assert!(output.contains("rendered output"));
        assert!(output.contains("Suggestion: try this fix"));
    }

    #[test]
    fn format_test_output_extracts_failure_sections() {
        let output = "test foo ... ok\n---- test_bar stdout ----\nthread panicked\ntest bar ... FAILED\ntest result: FAILED. 1 failed; 1 passed";
        let result = format_test_output_for_llm(output);
        assert!(result.contains("thread panicked"));
        assert!(result.contains("test bar ... FAILED"));
        assert!(!result.contains("test foo ... ok"));
    }
}
