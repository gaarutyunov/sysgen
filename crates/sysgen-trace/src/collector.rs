use std::path::Path;

/// Collects traceability annotations from Rust source files.
pub struct TraceCollector<'a> {
    src: &'a Path,
}

impl<'a> TraceCollector<'a> {
    pub fn new(src: &'a Path) -> Self {
        Self { src }
    }

    pub fn collect(&self) -> anyhow::Result<Vec<TraceEntry>> {
        let _ = self.src;
        todo!("implement trace collection")
    }
}

pub struct TraceEntry {
    pub id: String,
    pub location: String,
}
