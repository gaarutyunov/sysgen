use crate::parser::workspace::load_spec_manifest;
use crate::traceability::collector::collect_annotations;
use crate::traceability::report::{build_report, TraceabilityReport};

#[derive(clap::Args)]
pub struct CheckCommand {
    #[arg(long, default_value = "spec")]
    spec_dir: std::path::PathBuf,

    #[arg(long, default_value = "src")]
    src_dir: std::path::PathBuf,

    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    format: String,

    /// Exit with code 0 even if gaps exist (useful for CI reporting only)
    #[arg(long)]
    no_fail: bool,
}

impl CheckCommand {
    pub fn run(self) -> anyhow::Result<()> {
        println!("📖 Loading spec manifest from {:?}...", self.spec_dir);
        let manifest = load_spec_manifest(&self.spec_dir)?;

        println!("🔍 Scanning source annotations in {:?}...", self.src_dir);
        let annotations = collect_annotations(&self.src_dir)?;

        let report = build_report(&manifest, &annotations);

        match self.format.as_str() {
            "json" => println!("{}", serde_json::to_string_pretty(&report)?),
            _ => print_text_report(&report),
        }

        if !report.is_complete() && !self.no_fail {
            std::process::exit(1);
        }

        Ok(())
    }
}

fn print_text_report(report: &TraceabilityReport) {
    println!();
    println!("Traceability Report");
    println!("{}", "═".repeat(60));
    println!("{:<50} {:>5} {:>5}", "Requirement", "Impl", "Test");
    println!("{}", "─".repeat(60));

    for req in &report.requirements {
        let impl_mark = if req.implementations.is_empty() {
            "❌"
        } else {
            "✓ "
        };
        let test_mark = if req.verifications.is_empty() {
            "❌"
        } else {
            "✓ "
        };
        let short = req.short_name.as_deref().unwrap_or("—");
        let display = if req.req_id.len() > 45 {
            format!("{}...", &req.req_id[..42])
        } else {
            req.req_id.clone()
        };
        println!(
            "{:<50} {:>5} {:>5}  [{}]",
            display, impl_mark, test_mark, short
        );
    }

    println!("{}", "═".repeat(60));
    println!(
        "Coverage: {}/{} ({:.1}%) — {} impl missing, {} test missing",
        report.fully_covered,
        report.total_requirements,
        report.coverage_percent,
        report.missing_implementation + report.completely_missing,
        report.missing_verification + report.completely_missing,
    );

    if report.is_complete() {
        println!("\n✅ All requirements are implemented and verified.");
    } else {
        println!("\nRun `sysgen gen` to generate missing implementations.");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::parser::manifest::{RequirementDef, SpecManifest};
    use crate::traceability::collector::{CollectedAnnotations, ImplAnnotation, VerifyAnnotation};
    use crate::traceability::report::build_report;

    use super::*;

    fn make_manifest(reqs: Vec<(&str, Option<&str>)>) -> SpecManifest {
        let mut requirements = HashMap::new();
        let mut short_name_index = HashMap::new();
        for (qn, short) in reqs {
            let req = RequirementDef {
                qualified_name: qn.to_string(),
                short_name: short.map(str::to_string),
                doc: None,
                source_file: PathBuf::new(),
                satisfied_by: vec![],
                verified_by: vec![],
            };
            if let Some(s) = short {
                short_name_index.insert(s.to_string(), qn.to_string());
            }
            requirements.insert(qn.to_string(), req);
        }
        SpecManifest {
            requirements,
            short_name_index,
            source_files: vec![],
        }
    }

    fn impl_ann(req_id: &str) -> ImplAnnotation {
        ImplAnnotation {
            req_id: req_id.to_string(),
            item_name: "some_fn".to_string(),
            file: PathBuf::new(),
            line: 1,
        }
    }

    fn verify_ann(req_id: &str) -> VerifyAnnotation {
        VerifyAnnotation {
            req_id: req_id.to_string(),
            test_name: "some_test".to_string(),
            file: PathBuf::new(),
            line: 1,
        }
    }

    #[test]
    fn print_text_report_complete() {
        let manifest = make_manifest(vec![("Req::A", Some("R1"))]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A")],
            verifications: vec![verify_ann("Req::A")],
        };
        let report = build_report(&manifest, &annotations);
        // Should not panic
        print_text_report(&report);
        assert!(report.is_complete());
    }

    #[test]
    fn print_text_report_with_gaps() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations::default();
        let report = build_report(&manifest, &annotations);
        // Should not panic
        print_text_report(&report);
        assert!(!report.is_complete());
    }

    #[test]
    fn print_text_report_truncates_long_req_id() {
        let long_id = "A".repeat(50);
        let manifest = make_manifest(vec![(long_id.as_str(), None)]);
        let annotations = CollectedAnnotations::default();
        let report = build_report(&manifest, &annotations);
        // Should not panic with long req_id
        print_text_report(&report);
    }

    #[test]
    fn json_report_is_valid() {
        let manifest = make_manifest(vec![("Req::A", Some("R1")), ("Req::B", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A")],
            verifications: vec![verify_ann("Req::A")],
        };
        let report = build_report(&manifest, &annotations);
        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("Req::A"));
        assert!(json.contains("Req::B"));
    }
}
