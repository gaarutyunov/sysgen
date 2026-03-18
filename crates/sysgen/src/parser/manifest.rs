use std::collections::HashMap;
use std::path::PathBuf;

/// A single requirement extracted from a .sysml spec file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequirementDef {
    /// Fully qualified SysML name: "VehicleRequirements::MassRequirement"
    pub qualified_name: String,

    /// Short name from angle brackets: "R1" (from <'R1'>), None if absent
    pub short_name: Option<String>,

    /// Human-readable description from doc comment
    pub doc: Option<String>,

    /// Which .sysml file this was defined in
    pub source_file: PathBuf,

    /// Any satisfy relationships declared in the spec
    /// (design elements that declare they satisfy this req)
    pub satisfied_by: Vec<String>,

    /// Any verify relationships declared in the spec
    /// (verification defs that verify this req)
    pub verified_by: Vec<String>,
}

/// The complete parsed representation of all spec files in a project
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpecManifest {
    /// All requirements, keyed by qualified name
    pub requirements: HashMap<String, RequirementDef>,

    /// Short name → qualified name index for alias resolution
    pub short_name_index: HashMap<String, String>,

    /// All .sysml files that were parsed
    pub source_files: Vec<PathBuf>,
}

impl SpecManifest {
    /// Resolve either a qualified name or short name to a `RequirementDef`
    pub fn resolve(&self, id: &str) -> Option<&RequirementDef> {
        self.requirements.get(id).or_else(|| {
            self.short_name_index
                .get(id)
                .and_then(|qn| self.requirements.get(qn))
        })
    }

    /// Returns all requirement IDs (qualified names) sorted alphabetically
    pub fn all_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.requirements.keys().map(String::as_str).collect();
        ids.sort();
        ids
    }
}
