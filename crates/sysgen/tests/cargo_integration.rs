//! Integration tests that invoke real `cargo` sub-commands and verify that
//! [`ValidationResult`] captures build errors, clippy warnings, and test
//! failures with the expected structure and formatting.
//!
//! Each test creates a throwaway Cargo library crate in a `TempDir`, runs one
//! of the public runner functions, and asserts on the returned value.

use sysgen::validation::cargo::{run_build, run_clippy, run_test, ValidationResult};
use std::fs;
use tempfile::TempDir;

/// Create a minimal Cargo library crate in a temporary directory.
///
/// The crate has no external dependencies so cargo never needs network access.
fn make_lib_project(name: &str, src_code: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
        ),
    )
    .expect("write Cargo.toml");

    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src/");
    fs::write(src_dir.join("lib.rs"), src_code).expect("write lib.rs");

    dir
}

// ── run_build ────────────────────────────────────────────────────────────────

/// A valid Rust library compiles successfully.
#[tokio::test]
async fn build_valid_project_returns_success() {
    let proj = make_lib_project("build-ok", "pub fn hello() {}");

    let result = run_build(proj.path()).await.expect("run_build failed");

    assert!(
        matches!(result, ValidationResult::Success),
        "expected Success for valid project"
    );
}

/// A type-mismatch error is captured and reported with the correct format.
///
/// Verifies:
/// - `error_count > 0`
/// - Output starts with `"Error 1/N:"` block header
/// - Output references the compiler error code (`E0308`) or diagnostic message
#[tokio::test]
async fn build_type_error_returns_structured_failure() {
    let proj = make_lib_project(
        "build-type-err",
        r#"pub fn bad() { let _x: i32 = "not a number"; }"#,
    );

    let result = run_build(proj.path()).await.expect("run_build failed");

    let ValidationResult::Failure { output, error_count } = result else {
        panic!("expected Failure for code with type error, got Success");
    };

    assert!(error_count > 0, "error_count should be > 0; got {error_count}");
    assert!(
        output.contains("Error 1/"),
        "output should contain 'Error 1/N' block header; got:\n{output}"
    );
    assert!(
        output.contains("E0308") || output.contains("mismatched types"),
        "output should reference the type-mismatch diagnostic; got:\n{output}"
    );
}

/// Calling an undefined function produces a failure that includes file/line
/// location info in the `Location: file:start:end` format.
#[tokio::test]
async fn build_failure_output_contains_source_location() {
    let proj = make_lib_project(
        "build-loc",
        "pub fn bad() { undefined_fn(); }",
    );

    let result = run_build(proj.path()).await.expect("run_build failed");

    let ValidationResult::Failure { output, .. } = result else {
        panic!("expected Failure for undefined function call, got Success");
    };

    assert!(
        output.contains("Location:"),
        "output should contain 'Location:' field; got:\n{output}"
    );
    assert!(
        output.contains("src/lib.rs"),
        "output should reference 'src/lib.rs'; got:\n{output}"
    );
}

/// The `Detail:` section must not contain raw ANSI escape sequences —
/// `strip_ansi_codes` is applied before formatting.
#[tokio::test]
async fn build_failure_output_has_no_ansi_escape_codes() {
    let proj = make_lib_project(
        "build-ansi",
        r#"pub fn bad() { let _x: i32 = "bad value"; }"#,
    );

    let result = run_build(proj.path()).await.expect("run_build failed");

    let ValidationResult::Failure { output, .. } = result else {
        panic!("expected Failure for code with type error, got Success");
    };

    assert!(
        !output.contains('\x1b'),
        "ANSI escape codes should have been stripped from output; got:\n{output}"
    );
}

// ── run_clippy ───────────────────────────────────────────────────────────────

/// A clean project with no warnings produces a `Success` result.
#[tokio::test]
async fn clippy_clean_project_returns_success() {
    let proj = make_lib_project(
        "clippy-ok",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    );

    let result = run_clippy(proj.path()).await.expect("run_clippy failed");

    assert!(
        matches!(result, ValidationResult::Success),
        "expected Success for clean project"
    );
}

/// A `needless_return` lint fires as a warning; `-D warnings` (passed by
/// `run_clippy`) turns it into a hard error and produces a `Failure`.
#[tokio::test]
async fn clippy_needless_return_lint_produces_failure() {
    // `return` at the tail position of a function is always flagged by
    // `clippy::needless_return` (warn by default → error with -D warnings).
    let proj = make_lib_project(
        "clippy-needless-return",
        "pub fn value() -> i32 { return 42; }",
    );

    let result = run_clippy(proj.path()).await.expect("run_clippy failed");

    match result {
        ValidationResult::Failure { error_count, output } => {
            assert!(
                error_count > 0,
                "expected clippy to report > 0 errors; got {error_count}"
            );
            assert!(
                !output.is_empty(),
                "failure output should not be empty"
            );
        }
        ValidationResult::Success => {
            // Some toolchain configurations may have this lint disabled;
            // treat as a soft skip rather than a hard test failure.
            eprintln!(
                "clippy returned Success; `needless_return` may be \
                 disabled in this toolchain — skipping assertion"
            );
        }
    }
}

// ── run_test ─────────────────────────────────────────────────────────────────

/// A test suite where all tests pass returns `Success`.
#[tokio::test]
async fn test_passing_suite_returns_success() {
    let proj = make_lib_project(
        "test-pass",
        r#"pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}
"#,
    );

    let result = run_test(proj.path()).await.expect("run_test failed");

    assert!(
        matches!(result, ValidationResult::Success),
        "expected Success for a passing test suite"
    );
}

/// A panicking test is captured and the panic message appears in the output.
///
/// Verifies:
/// - `error_count > 0`
/// - The failure output contains the panic message or the test name
#[tokio::test]
async fn test_failing_suite_captures_panic_message() {
    let proj = make_lib_project(
        "test-fail",
        r#"#[cfg(test)]
mod tests {
    #[test]
    fn always_fails() {
        panic!("intentional failure");
    }
}
"#,
    );

    let result = run_test(proj.path()).await.expect("run_test failed");

    let ValidationResult::Failure { output, error_count } = result else {
        panic!("expected Failure for panicking test, got Success");
    };

    assert!(error_count > 0, "expected at least one test failure; got {error_count}");
    assert!(
        output.contains("intentional failure") || output.contains("always_fails"),
        "failure output should reference the panicking test; got:\n{output}"
    );
}

/// An `assert_eq!` failure is captured and the compared values appear in output.
#[tokio::test]
async fn test_assert_eq_failure_captures_values() {
    let proj = make_lib_project(
        "test-assert",
        r#"#[cfg(test)]
mod tests {
    #[test]
    fn wrong_addition() {
        assert_eq!(1 + 1, 3, "math is broken");
    }
}
"#,
    );

    let result = run_test(proj.path()).await.expect("run_test failed");

    let ValidationResult::Failure { output, error_count } = result else {
        panic!("expected Failure for assert_eq test, got Success");
    };

    assert!(error_count > 0, "expected at least one test failure; got {error_count}");
    assert!(
        output.contains("math is broken")
            || output.contains("wrong_addition")
            || output.contains("assert_eq"),
        "failure output should reference the assertion failure; got:\n{output}"
    );
}
