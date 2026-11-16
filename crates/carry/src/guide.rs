//! The project's guide for its agents on a work's branch, as the data of the source [`GUIDE`], read in the step that
//! reads the work's branch (`crate::branch`): a new version each time its text differs from the last version, the text
//! its content (design document `data-and-context`, 24). What changed between two versions is what differs in their
//! texts, which the context manager tells itself; the engine compares none of them.

use crate::git::{self, Place};

/// The source outside the work that writes the block of the project's guide.
pub const GUIDE: &str = "guide";

/// The file of the project's declaration (`[files]` of `flow/project.toml`) that the guide is.
pub const FILE: &str = "guide";

/// The content of the new version the work's branch makes of the guide at `path`, after `last`, the latest there is,
/// where it makes one: its text, where it differs from `last`. A branch that has no such file makes none: the engine
/// does not start where main lacks the files the project's declaration names (`crate::system::project_files`).
pub(crate) async fn read(place: &Place, path: &str, last: Option<&str>) -> Result<Option<String>, String> {
    let spec = format!("{}:{path}", place.reference());
    let said = git::said(&place.dir, &["show", &spec]).await?;
    Ok((said.ok && last != Some(said.out.as_str())).then_some(said.out))
}
