/// SysML manifest metadata (workspace name and configuration).
pub struct SysmlManifestParser {
    name: String,
}

impl SysmlManifestParser {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
