use crate::parser::manifest::SpecManifest;
use anyhow::Result;
use std::path::Path;

pub struct PromptContext {
    pub manifest: SpecManifest,
    pub project_root: std::path::PathBuf,
    pub target_language: String, // "rust" (only rust for now)
}

pub fn build_initial_message(ctx: &PromptContext) -> Result<String> {
    let mut msg = String::new();

    // ── Section 1: Role ──────────────────────────────────────
    msg.push_str("# SysGen Code Generation Task\n\n");
    msg.push_str(
        "You are a Rust software engineer implementing code driven by a SysML v2 specification.\n",
    );
    msg.push_str("Your job is to write Rust implementation code and tests that:\n");
    msg.push_str("1. Implement every requirement listed below\n");
    msg.push_str("2. Annotate each implementation with `#[implements(\"QualifiedName\")]`\n");
    msg.push_str(
        "3. Write a test for each requirement annotated with `#[verifies(\"QualifiedName\")]`\n",
    );
    msg.push_str(
        "4. Ensure `cargo build`, `cargo clippy -- -D warnings`, and `cargo test` all pass\n\n",
    );

    // ── Section 2: CRITICAL rules ────────────────────────────
    msg.push_str("## CRITICAL RULES — Violations will abort the run\n\n");
    msg.push_str(
        "- ❌ NEVER write to or modify any file under `spec/`. Spec files are READ-ONLY.\n",
    );
    msg.push_str(
        "- ❌ NEVER remove or alter existing `#[implements]` or `#[verifies]` annotations.\n",
    );
    msg.push_str("- ✅ Only write to files under `src/` and `tests/`.\n");
    msg.push_str("- ✅ Every `requirement def` in the spec MUST have at least one `#[implements]` and one `#[verifies]`.\n\n");

    // ── Section 3: Annotation reference ──────────────────────
    msg.push_str("## Annotation syntax\n\n");
    msg.push_str("```rust\n");
    msg.push_str("use sysgen_macros::{implements, verifies};\n\n");
    msg.push_str("// Implementation annotation (on fn, struct, impl block, or mod)\n");
    msg.push_str("#[implements(\"VehicleRequirements::MassRequirement\")]\n");
    msg.push_str("pub fn check_mass(vehicle: &Vehicle) -> bool {\n");
    msg.push_str("    vehicle.mass <= MASS_LIMIT\n");
    msg.push_str("}\n\n");
    msg.push_str("// Test annotation (MUST be on a #[test] function)\n");
    msg.push_str("#[verifies(\"VehicleRequirements::MassRequirement\")]\n");
    msg.push_str("#[test]\n");
    msg.push_str("fn test_mass_within_limit() {\n");
    msg.push_str("    let v = Vehicle { mass: 1500.0 };\n");
    msg.push_str("    assert!(check_mass(&v));\n");
    msg.push_str("}\n");
    msg.push_str("```\n\n");

    // ── Section 4: Requirements table ────────────────────────
    msg.push_str("## Requirements to implement\n\n");
    msg.push_str("| ID | Short Name | Description |\n");
    msg.push_str("|-----|-----------|-------------|\n");
    let mut sorted_reqs: Vec<_> = ctx.manifest.requirements.values().collect();
    sorted_reqs.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
    for req in sorted_reqs {
        msg.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            req.qualified_name,
            req.short_name.as_deref().unwrap_or("—"),
            req.doc
                .as_deref()
                .unwrap_or("(no doc)")
                .lines()
                .next()
                .unwrap_or("") // First line only for brevity
        ));
    }
    msg.push('\n');

    // ── Section 5: Project structure ─────────────────────────
    msg.push_str("## Project structure\n\n");
    msg.push_str("```\n");
    msg.push_str(&format!("{}\n", describe_project_tree(&ctx.project_root)?));
    msg.push_str("```\n\n");

    // ── Section 6: Cargo.toml ─────────────────────────────────
    let cargo_toml_path = ctx.project_root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        msg.push_str("## Cargo.toml\n\n```toml\n");
        msg.push_str(&std::fs::read_to_string(&cargo_toml_path)?);
        msg.push_str("```\n\n");
    }

    // ── Section 7: Action ─────────────────────────────────────
    msg.push_str("## Your task\n\n");
    msg.push_str("1. Write Rust code in `src/` implementing all requirements above\n");
    msg.push_str(
        "2. Write tests in `tests/` or `src/` (with `#[cfg(test)]`) verifying each requirement\n",
    );
    msg.push_str(
        "3. After writing code, run: `cargo build && cargo clippy -- -D warnings && cargo test`\n",
    );
    msg.push_str("4. Fix any errors and re-run until all three pass\n");
    msg.push_str(
        "5. Check traceability: every requirement must have `#[implements]` AND `#[verifies]`\n\n",
    );
    msg.push_str(
        "Start by reading the existing project structure, then implement requirement by requirement.\n",
    );

    Ok(msg)
}

/// Build a re-prompt message when traceability gaps remain after agent completion
pub fn build_gap_reprompt(gap_message: &str, iteration: u32, max_iterations: u32) -> String {
    format!(
        "# Traceability Gaps Detected (Iteration {}/{})\n\n\
         The previous implementation attempt is incomplete. `cargo build`, `cargo clippy`, \
         and `cargo test` all pass, but the following requirements still lack traceability annotations:\n\n\
         {}\n\n\
         Please add the missing `#[implements]` and/or `#[verifies]` annotations for each gap listed above.\n\
         Remember: the annotation string must exactly match the qualified name shown.\n\
         After adding annotations, verify `cargo test` still passes.",
        iteration, max_iterations, gap_message
    )
}

/// Build re-prompt when build/clippy/test fails
pub fn build_error_reprompt(stage: &str, error_output: &str, iteration: u32) -> String {
    format!(
        "# Build Failure (Iteration {}) — Stage: {}\n\n\
         The following errors need to be fixed:\n\n\
         ```\n{}\n```\n\n\
         Fix these errors, then re-run `cargo build && cargo clippy -- -D warnings && cargo test`.",
        iteration, stage, error_output
    )
}

fn describe_project_tree(root: &Path) -> Result<String> {
    let mut tree = String::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Skip hidden dirs and target/ by checking the entry's own name only
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "target")
        })
    {
        let depth = entry.depth();
        let indent = "  ".repeat(depth);
        let name = entry.file_name().to_string_lossy();
        tree.push_str(&format!("{}{}\n", indent, name));
    }
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::manifest::{RequirementDef, SpecManifest};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_manifest(reqs: Vec<RequirementDef>) -> SpecManifest {
        let mut requirements = HashMap::new();
        let mut short_name_index = HashMap::new();
        for req in reqs {
            if let Some(sn) = &req.short_name {
                short_name_index.insert(sn.clone(), req.qualified_name.clone());
            }
            requirements.insert(req.qualified_name.clone(), req);
        }
        SpecManifest {
            requirements,
            short_name_index,
            source_files: vec![],
        }
    }

    fn make_req(qname: &str, short: Option<&str>, doc: Option<&str>) -> RequirementDef {
        RequirementDef {
            qualified_name: qname.to_string(),
            short_name: short.map(str::to_string),
            doc: doc.map(str::to_string),
            source_file: PathBuf::from("spec/test.sysml"),
            satisfied_by: vec![],
            verified_by: vec![],
        }
    }

    #[test]
    fn build_initial_message_contains_requirement_names() {
        let manifest = make_manifest(vec![make_req(
            "Pkg::ReqA",
            Some("R1"),
            Some("A test requirement"),
        )]);
        let ctx = PromptContext {
            manifest,
            project_root: std::env::temp_dir(),
            target_language: "rust".to_string(),
        };
        let msg = build_initial_message(&ctx).expect("should build message");
        assert!(msg.contains("Pkg::ReqA"), "qualified name must appear");
        assert!(msg.contains("R1"), "short name must appear");
        assert!(msg.contains("A test requirement"), "doc must appear");
    }

    #[test]
    fn build_initial_message_contains_critical_rules() {
        let manifest = make_manifest(vec![]);
        let ctx = PromptContext {
            manifest,
            project_root: std::env::temp_dir(),
            target_language: "rust".to_string(),
        };
        let msg = build_initial_message(&ctx).expect("should build");
        assert!(
            msg.contains("CRITICAL RULES"),
            "must have critical rules section"
        );
        assert!(msg.contains("spec/"), "must mention spec/ prohibition");
        assert!(
            msg.contains("#[implements]"),
            "must mention implements annotation"
        );
        assert!(
            msg.contains("#[verifies]"),
            "must mention verifies annotation"
        );
    }

    #[test]
    fn build_initial_message_contains_annotation_syntax() {
        let manifest = make_manifest(vec![]);
        let ctx = PromptContext {
            manifest,
            project_root: std::env::temp_dir(),
            target_language: "rust".to_string(),
        };
        let msg = build_initial_message(&ctx).expect("should build");
        assert!(msg.contains("sysgen_macros"), "must show macro import");
        assert!(
            msg.contains("VehicleRequirements::MassRequirement"),
            "must show annotation example"
        );
    }

    #[test]
    fn build_initial_message_includes_cargo_toml_when_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cargo_path = dir.path().join("Cargo.toml");
        std::fs::write(&cargo_path, "[package]\nname = \"test\"\n").expect("write Cargo.toml");

        let manifest = make_manifest(vec![]);
        let ctx = PromptContext {
            manifest,
            project_root: dir.path().to_path_buf(),
            target_language: "rust".to_string(),
        };
        let msg = build_initial_message(&ctx).expect("should build");
        assert!(
            msg.contains("Cargo.toml"),
            "must include Cargo.toml section"
        );
        assert!(
            msg.contains("[package]"),
            "must include Cargo.toml contents"
        );
    }

    #[test]
    fn build_gap_reprompt_contains_iteration_and_message() {
        let reprompt = build_gap_reprompt("Pkg::MissingReq", 2, 5);
        assert!(reprompt.contains("2/5"), "must show iteration count");
        assert!(
            reprompt.contains("Pkg::MissingReq"),
            "must include gap message"
        );
        assert!(
            reprompt.contains("#[implements]"),
            "must mention implements"
        );
        assert!(reprompt.contains("#[verifies]"), "must mention verifies");
    }

    #[test]
    fn build_error_reprompt_contains_stage_and_output() {
        let reprompt = build_error_reprompt("clippy", "error[E0001]: unused import", 3);
        assert!(reprompt.contains("clippy"), "must mention stage");
        assert!(
            reprompt.contains("error[E0001]: unused import"),
            "must include error output"
        );
        assert!(reprompt.contains("Iteration 3"), "must mention iteration");
        assert!(reprompt.contains("```"), "must wrap error in code block");
    }

    #[test]
    fn describe_project_tree_skips_hidden_and_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join(".git")).expect("mkdir .git");
        std::fs::create_dir(dir.path().join("target")).expect("mkdir target");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir src");
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").expect("write");

        let tree = describe_project_tree(dir.path()).expect("should work");
        assert!(!tree.contains(".git"), "should skip hidden dirs");
        assert!(!tree.contains("target"), "should skip target/");
        assert!(tree.contains("src"), "should include src/");
        assert!(tree.contains("main.rs"), "should include source files");
    }
}
