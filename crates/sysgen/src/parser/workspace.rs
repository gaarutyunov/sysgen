use std::path::{Path, PathBuf};

/// SysML workspace pointing to a directory containing .sysml spec files.
pub struct SysmlWorkspaceParser {
    path: PathBuf,
}

impl SysmlWorkspaceParser {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
