/// Stub implementation of syster-base.
/// Replace with the real git dependency once the commit SHA is known.

use std::path::PathBuf;

pub struct SysmlWorkspace {
    pub path: PathBuf,
}

impl SysmlWorkspace {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

pub struct SysmlManifest {
    pub name: String,
}

impl SysmlManifest {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
