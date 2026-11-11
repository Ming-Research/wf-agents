//! The local repository as the host: main is its own main branch, the verdict is the project's check command run in
//! the work's worktree, and the merge is made in its checkout.

use super::{Branch, Host, Verdict};
use crate::git;
use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The project's local repository, as its own host.
#[derive(Clone, Debug)]
pub struct Local {
    project: Project,
    /// The variables of the engine's environment the check command is given beside those always given.
    env: Vec<String>,
}

impl Local {
    pub fn new(project: Project) -> Local {
        Local {
            project,
            env: Vec::new(),
        }
    }

    /// The host whose check command is given these variables of the engine's environment, as the agents are.
    pub fn giving(mut self, env: Vec<String>) -> Local {
        self.env = env;
        self
    }
}

impl Host for Local {
    async fn main(&self) -> Result<String, String> {
        let main = format!("refs/heads/{}", self.project.main);
        git::git(&self.project.repo, &["rev-parse", "--verify", &main]).await
    }

    async fn verdict(&self, branch: &Branch<'_>, _: &str, stop: &mut Stop) -> Result<Option<Verdict>, String> {
        let dir = git::worktree(branch.place)?;
        let check = &self.project.check;
        let failed = match process::run(process::shell(check, &dir, &self.env), None, None, stop).await? {
            Ran::Ended { status, stdout, .. } if !status.success() => Some(format!(
                "The check `{check}` failed ({status}). The end of what it said:\n{}",
                process::tail(&stdout)
            )),
            Ran::Ended { .. } => None,
            Ran::Stopped => return Ok(None),
        };
        Ok(Some(Verdict { failed, by: None }))
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, String> {
        git::merge(&self.project, branch.place, checked).await
    }
}
