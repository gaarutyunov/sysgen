/// Wraps syster-base workspace parsing.
pub struct SysmlWorkspaceParser {
    inner: syster_base::SysmlWorkspace,
}

impl SysmlWorkspaceParser {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            inner: syster_base::SysmlWorkspace::new(path),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.inner.path
    }
}
