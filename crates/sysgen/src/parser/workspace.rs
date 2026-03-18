use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use syster::hir::{RelationshipKind, SymbolKind, TypeRefKind};
use syster::ide::AnalysisHost;
use syster::project::{StdLibLoader, WorkspaceLoader};
use walkdir::WalkDir;

use crate::parser::manifest::{RequirementDef, SpecManifest};

/// Load and parse all `.sysml` files in `spec_dir`, returning a `SpecManifest`
/// containing every `requirement def` found, with short-name aliases and
/// any `satisfy`/`verify` relationships populated.
pub fn load_spec_manifest(spec_dir: &Path) -> Result<SpecManifest> {
    let mut host = AnalysisHost::new();

    // Load SysML v2 standard library (bundled with syster).
    // Failures are non-fatal: symbol resolution still works for local definitions.
    if let Err(e) = StdLibLoader::new().ensure_loaded_into_host(&mut host) {
        tracing::warn!(
            "Could not load SysML v2 stdlib (continuing without it): {}",
            e
        );
    }

    // Load all .sysml files from the spec directory recursively.
    WorkspaceLoader::new()
        .load_directory_into_host(spec_dir, &mut host)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to load spec files from {}: {}",
                spec_dir.display(),
                e
            )
        })?;

    let analysis = host.analysis();
    let index = analysis.symbol_index();

    let mut manifest = SpecManifest {
        requirements: HashMap::new(),
        short_name_index: HashMap::new(),
        source_files: Vec::new(),
    };

    // Collect all .sysml source files present in the spec directory.
    manifest.source_files = WalkDir::new(spec_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "sysml")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    // First pass: collect all `requirement def` symbols.
    for symbol in index.all_symbols() {
        if symbol.kind != SymbolKind::RequirementDefinition {
            continue;
        }

        let source_file = analysis
            .get_file_path(symbol.file)
            .map(PathBuf::from)
            .unwrap_or_default();

        let qn = symbol.qualified_name.to_string();
        let req = RequirementDef {
            qualified_name: qn.clone(),
            short_name: symbol.short_name.as_deref().map(str::to_owned),
            doc: symbol.doc.as_deref().map(str::to_owned),
            source_file,
            satisfied_by: Vec::new(),
            verified_by: Vec::new(),
        };

        if let Some(short) = &req.short_name {
            manifest.short_name_index.insert(short.clone(), qn.clone());
        }

        manifest.requirements.insert(qn, req);
    }

    // Second pass: populate satisfy/verify relationships.
    //
    // Two complementary strategies are used:
    //
    // 1. `HirRelationship` entries with `Satisfies`/`Verifies` kinds — covers the
    //    standalone `satisfy X by Y` SysML syntax where syster emits explicit edges.
    //
    // 2. Anonymous `RequirementUsage` child symbols — covers the inline
    //    `satisfy requirement : Req` syntax inside a PartDef/VerificationCaseDef.
    //    syster-base represents this as a child symbol whose `type_refs` point to
    //    the satisfied/verified requirement.
    for symbol in index.all_symbols() {
        let satisfier_qn = symbol.qualified_name.to_string();

        // Strategy 1: explicit relationship edges.
        for rel in &symbol.relationships {
            let req_target = rel
                .resolved_target
                .as_deref()
                .unwrap_or(rel.target.as_ref())
                .to_string();

            match rel.kind {
                RelationshipKind::Satisfies => {
                    if let Some(req) = manifest.requirements.get_mut(&req_target) {
                        if !req.satisfied_by.contains(&satisfier_qn) {
                            req.satisfied_by.push(satisfier_qn.clone());
                        }
                    }
                }
                RelationshipKind::Verifies => {
                    if let Some(req) = manifest.requirements.get_mut(&req_target) {
                        if !req.verified_by.contains(&satisfier_qn) {
                            req.verified_by.push(satisfier_qn.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Strategy 2: anonymous RequirementUsage child symbols.
        if symbol.kind != SymbolKind::RequirementUsage {
            continue;
        }

        let qn = symbol.qualified_name.as_ref();
        let Some(sep) = qn.rfind("::") else { continue };
        let parent_qn = &qn[..sep];

        // Extract parent kind and name as owned values to avoid borrow conflicts
        // when mutating `manifest` below.
        let Some((parent_kind, parent_name)) = index
            .lookup_qualified(parent_qn)
            .map(|p| (p.kind, parent_qn.to_string()))
        else {
            continue;
        };

        for type_ref in &symbol.type_refs {
            let target = match type_ref {
                TypeRefKind::Simple(r) => r
                    .resolved_target
                    .as_deref()
                    .unwrap_or(r.target.as_ref())
                    .to_string(),
                TypeRefKind::Chain(_) => continue,
            };

            if let Some(req) = manifest.requirements.get_mut(&target) {
                match parent_kind {
                    SymbolKind::PartDefinition
                    | SymbolKind::PartUsage
                    | SymbolKind::ItemDefinition
                    | SymbolKind::ItemUsage => {
                        if !req.satisfied_by.contains(&parent_name) {
                            req.satisfied_by.push(parent_name.clone());
                        }
                    }
                    SymbolKind::VerificationCaseDefinition => {
                        if !req.verified_by.contains(&parent_name) {
                            req.verified_by.push(parent_name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn loads_requirements_from_fixture() {
        let manifest = load_spec_manifest(Path::new("tests/fixtures/"))
            .expect("manifest load failed");

        assert_eq!(manifest.requirements.len(), 3);
        assert!(manifest
            .requirements
            .contains_key("VehicleRequirements::MassRequirement"));
        assert_eq!(
            manifest.resolve("R1").unwrap().qualified_name,
            "VehicleRequirements::MassRequirement"
        );
    }

    #[test]
    fn short_name_aliases_resolve() {
        let manifest = load_spec_manifest(Path::new("tests/fixtures/"))
            .expect("manifest load failed");

        assert!(manifest.resolve("R1").is_some());
        assert!(manifest.resolve("R2").is_some());
        assert!(manifest.resolve("R3").is_some());
    }
}
