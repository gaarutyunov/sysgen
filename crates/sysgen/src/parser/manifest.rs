/// Wraps syster-base manifest parsing.
pub struct SysmlManifestParser {
    inner: syster_base::SysmlManifest,
}

impl SysmlManifestParser {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: syster_base::SysmlManifest::new(name),
        }
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }
}
