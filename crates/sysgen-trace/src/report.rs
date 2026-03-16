use std::path::PathBuf;

use crate::collector::TraceEntry;

/// Generates a traceability report mapping spec elements to code.
pub struct TraceReporter {
    spec: PathBuf,
}

impl TraceReporter {
    pub fn new(spec: PathBuf) -> Self {
        Self { spec }
    }

    pub fn report(&self, entries: &[TraceEntry]) -> anyhow::Result<()> {
        let _ = (&self.spec, entries);
        todo!("implement trace reporter")
    }
}
