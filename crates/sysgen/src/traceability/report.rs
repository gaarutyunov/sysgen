use std::collections::HashMap;

use crate::parser::manifest::SpecManifest;
use crate::traceability::collector::{CollectedAnnotations, ImplAnnotation, VerifyAnnotation};

#[derive(Debug, serde::Serialize)]
pub enum CoverageStatus {
    /// Both implementation and test present
    Full,
    /// Implementation present, test missing
    NotVerified,
    /// Test present, implementation missing
    NotImplemented,
    /// Neither implementation nor test
    Missing,
}

#[derive(Debug, serde::Serialize)]
pub struct RequirementCoverage {
    pub req_id: String,
    pub short_name: Option<String>,
    pub doc: Option<String>,
    pub status: CoverageStatus,
    pub implementations: Vec<ImplAnnotation>,
    pub verifications: Vec<VerifyAnnotation>,
}

#[derive(Debug, serde::Serialize)]
pub struct TraceabilityReport {
    pub total_requirements: usize,
    pub fully_covered: usize,
    pub missing_implementation: usize,
    pub missing_verification: usize,
    pub completely_missing: usize,
    pub coverage_percent: f64,
    pub requirements: Vec<RequirementCoverage>,
}

impl TraceabilityReport {
    pub fn is_complete(&self) -> bool {
        self.fully_covered == self.total_requirements
    }

    /// Returns requirements that are not fully covered — used to build the LLM re-prompt.
    pub fn gaps(&self) -> Vec<&RequirementCoverage> {
        self.requirements
            .iter()
            .filter(|r| !matches!(r.status, CoverageStatus::Full))
            .collect()
    }

    /// Format gaps as a human-readable message for the LLM agent re-prompt.
    pub fn format_gap_message(&self) -> String {
        if self.is_complete() {
            return "All requirements are implemented and verified.".to_string();
        }

        let mut msg = format!(
            "TRACEABILITY GAPS DETECTED: {}/{} requirements fully covered.\n\n",
            self.fully_covered, self.total_requirements
        );

        for gap in self.gaps() {
            msg.push_str(&format!(
                "❌ {} ({})\n",
                gap.req_id,
                gap.short_name.as_deref().unwrap_or("no short name")
            ));
            msg.push_str(&format!(
                "   Doc: {}\n",
                gap.doc.as_deref().unwrap_or("(no doc)")
            ));
            match &gap.status {
                CoverageStatus::Missing => {
                    msg.push_str("   ⚠ Missing BOTH #[implements] and #[verifies]\n");
                    msg.push_str("   → Add a function annotated #[implements(\"");
                    msg.push_str(&gap.req_id);
                    msg.push_str("\")]\n");
                    msg.push_str("   → Add a test annotated #[verifies(\"");
                    msg.push_str(&gap.req_id);
                    msg.push_str("\")]\n");
                }
                CoverageStatus::NotImplemented => {
                    msg.push_str("   ⚠ Missing #[implements] annotation\n");
                    msg.push_str(&format!(
                        "   Verified by: {:?}\n",
                        gap.verifications
                            .iter()
                            .map(|v| &v.test_name)
                            .collect::<Vec<_>>()
                    ));
                }
                CoverageStatus::NotVerified => {
                    msg.push_str("   ⚠ Missing #[verifies] annotation (test needed)\n");
                    msg.push_str(&format!(
                        "   Implemented by: {:?}\n",
                        gap.implementations
                            .iter()
                            .map(|i| &i.item_name)
                            .collect::<Vec<_>>()
                    ));
                }
                CoverageStatus::Full => unreachable!(),
            }
            msg.push('\n');
        }

        msg
    }
}

pub fn build_report(
    manifest: &SpecManifest,
    annotations: &CollectedAnnotations,
) -> TraceabilityReport {
    let mut impl_by_req: HashMap<String, Vec<ImplAnnotation>> = HashMap::new();
    let mut verify_by_req: HashMap<String, Vec<VerifyAnnotation>> = HashMap::new();

    for ann in &annotations.implementations {
        let qn = manifest
            .resolve(&ann.req_id)
            .map(|r| r.qualified_name.clone())
            .unwrap_or_else(|| ann.req_id.clone());
        impl_by_req.entry(qn).or_default().push(ann.clone());
    }

    for ann in &annotations.verifications {
        let qn = manifest
            .resolve(&ann.req_id)
            .map(|r| r.qualified_name.clone())
            .unwrap_or_else(|| ann.req_id.clone());
        verify_by_req.entry(qn).or_default().push(ann.clone());
    }

    let mut requirements: Vec<RequirementCoverage> = manifest
        .requirements
        .values()
        .map(|req| {
            let impls = impl_by_req
                .get(&req.qualified_name)
                .cloned()
                .unwrap_or_default();
            let verifs = verify_by_req
                .get(&req.qualified_name)
                .cloned()
                .unwrap_or_default();

            let status = match (impls.is_empty(), verifs.is_empty()) {
                (false, false) => CoverageStatus::Full,
                (false, true) => CoverageStatus::NotVerified,
                (true, false) => CoverageStatus::NotImplemented,
                (true, true) => CoverageStatus::Missing,
            };

            RequirementCoverage {
                req_id: req.qualified_name.clone(),
                short_name: req.short_name.clone(),
                doc: req.doc.clone(),
                status,
                implementations: impls,
                verifications: verifs,
            }
        })
        .collect();

    requirements.sort_by(|a, b| a.req_id.cmp(&b.req_id));

    let fully_covered = requirements
        .iter()
        .filter(|r| matches!(r.status, CoverageStatus::Full))
        .count();
    let missing_impl = requirements
        .iter()
        .filter(|r| matches!(r.status, CoverageStatus::NotImplemented))
        .count();
    let missing_verify = requirements
        .iter()
        .filter(|r| matches!(r.status, CoverageStatus::NotVerified))
        .count();
    let completely_missing = requirements
        .iter()
        .filter(|r| matches!(r.status, CoverageStatus::Missing))
        .count();
    let total = requirements.len();

    TraceabilityReport {
        total_requirements: total,
        fully_covered,
        missing_implementation: missing_impl,
        missing_verification: missing_verify,
        completely_missing,
        coverage_percent: if total == 0 {
            100.0
        } else {
            (fully_covered as f64 / total as f64) * 100.0
        },
        requirements,
    }
}

pub fn print_text_report(report: &TraceabilityReport) {
    println!("=== Traceability Report ===");
    println!("Total requirements : {}", report.total_requirements);
    println!("Fully covered      : {}", report.fully_covered);
    println!("Missing impl       : {}", report.missing_implementation);
    println!("Missing verify     : {}", report.missing_verification);
    println!("Completely missing : {}", report.completely_missing);
    println!("Coverage           : {:.1}%", report.coverage_percent);
    println!();

    if !report.is_complete() {
        print!("{}", report.format_gap_message());
    } else {
        println!("✓ All requirements are fully covered.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::parser::manifest::{RequirementDef, SpecManifest};
    use crate::traceability::collector::{CollectedAnnotations, ImplAnnotation, VerifyAnnotation};

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
    fn status_full_when_both_present() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A")],
            verifications: vec![verify_ann("Req::A")],
        };
        let report = build_report(&manifest, &annotations);
        assert_eq!(report.fully_covered, 1);
        assert!(matches!(
            report.requirements[0].status,
            CoverageStatus::Full
        ));
    }

    #[test]
    fn status_not_verified_when_impl_only() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A")],
            verifications: vec![],
        };
        let report = build_report(&manifest, &annotations);
        assert_eq!(report.missing_verification, 1);
        assert!(matches!(
            report.requirements[0].status,
            CoverageStatus::NotVerified
        ));
    }

    #[test]
    fn status_not_implemented_when_verify_only() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![],
            verifications: vec![verify_ann("Req::A")],
        };
        let report = build_report(&manifest, &annotations);
        assert_eq!(report.missing_implementation, 1);
        assert!(matches!(
            report.requirements[0].status,
            CoverageStatus::NotImplemented
        ));
    }

    #[test]
    fn status_missing_when_neither_present() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations::default();
        let report = build_report(&manifest, &annotations);
        assert_eq!(report.completely_missing, 1);
        assert!(matches!(
            report.requirements[0].status,
            CoverageStatus::Missing
        ));
    }

    #[test]
    fn short_name_resolution() {
        let manifest = make_manifest(vec![("VehicleRequirements::MassRequirement", Some("R1"))]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("R1")],
            verifications: vec![verify_ann("R1")],
        };
        let report = build_report(&manifest, &annotations);
        assert_eq!(report.fully_covered, 1);
        assert_eq!(
            report.requirements[0].req_id,
            "VehicleRequirements::MassRequirement"
        );
    }

    #[test]
    fn coverage_percent_is_100_when_all_covered() {
        let manifest = make_manifest(vec![("Req::A", None), ("Req::B", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A"), impl_ann("Req::B")],
            verifications: vec![verify_ann("Req::A"), verify_ann("Req::B")],
        };
        let report = build_report(&manifest, &annotations);
        assert!((report.coverage_percent - 100.0).abs() < f64::EPSILON);
        assert!(report.is_complete());
    }

    #[test]
    fn coverage_percent_is_100_for_empty_manifest() {
        let manifest = make_manifest(vec![]);
        let annotations = CollectedAnnotations::default();
        let report = build_report(&manifest, &annotations);
        assert!((report.coverage_percent - 100.0).abs() < f64::EPSILON);
        assert!(report.is_complete());
    }

    #[test]
    fn format_gap_message_complete() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations {
            implementations: vec![impl_ann("Req::A")],
            verifications: vec![verify_ann("Req::A")],
        };
        let report = build_report(&manifest, &annotations);
        assert_eq!(
            report.format_gap_message(),
            "All requirements are implemented and verified."
        );
    }

    #[test]
    fn format_gap_message_with_gaps() {
        let manifest = make_manifest(vec![("Req::A", None)]);
        let annotations = CollectedAnnotations::default();
        let report = build_report(&manifest, &annotations);
        let msg = report.format_gap_message();
        assert!(msg.contains("TRACEABILITY GAPS DETECTED"));
        assert!(msg.contains("Req::A"));
        assert!(msg.contains("#[implements]"));
        assert!(msg.contains("#[verifies]"));
    }
}
