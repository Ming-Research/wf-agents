//! The local repository as the host: main is its own main branch, the verdict is the project's check command run in
//! the work's worktree, and the merge is made in its checkout.

use super::{Host, Verdict};
use crate::git::{self, Place};
use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The project's local repository, as its own host.
#[derive(Clone, Debug)]
pub struct Local {
    project: Project,
}

impl Local {
    pub fn new(project: Project) -> Local {
        Local { project }
    }
}

impl Host for Local {
    async fn main(&self) -> Result<String, String> {
        let main = format!("refs/heads/{}", self.project.main);
        git::git(&self.project.repo, &["rev-parse", "--verify", &main]).await
    }

    async fn verdict(&self, place: &Place, _: &str, stop: &mut Stop) -> Result<Option<Verdict>, String> {
        let dir = git::worktree(place)?;
        let check = &self.project.check;
        let failed = match process::run(process::shell(check, &dir), None, None, stop).await? {
            Ran::Ended { status, stdout, .. } if !status.success() => Some(format!(
                "The check `{check}` failed ({status}). The end of what it said:\n{}",
                process::tail(&stdout)
            )),
            Ran::Ended { .. } => None,
            Ran::Stopped => return Ok(None),
        };
        Ok(Some(Verdict { failed, by: None }))
    }

    async fn merge(&self, place: &Place, checked: Option<&str>) -> Result<String, String> {
        git::merge(&self.project, place, checked).await
    }
}
