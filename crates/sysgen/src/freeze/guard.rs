use std::path::PathBuf;

/// Prevents modifications to frozen spec files during generation.
pub struct FreezeGuard {
    pub path: PathBuf,
}

impl FreezeGuard {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn acquire(&self) -> anyhow::Result<()> {
        todo!("implement freeze guard")
    }
}
