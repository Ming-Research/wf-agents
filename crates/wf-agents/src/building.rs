//! `wf-agents play --states`: the build of every state of the page (design document `testing`, 16 to 21), not built
//! yet.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Builds nothing yet, and fails.
pub fn build(data: &Path, dirs: &[PathBuf], items: &Path, start: Option<u64>) -> ExitCode {
    let _ = (data, dirs, items, start);
    eprintln!("the build of the page's states is not built");
    ExitCode::FAILURE
}
