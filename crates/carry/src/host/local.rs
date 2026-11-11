//! The local repository as the host: main is its own main branch, the verdict is the project's check command run in
//! the work's worktree, and the merge is made in its checkout. An agent's check runs the same command on the worktree
//! as it stands, what is not committed included, and says the end of what it said, whether it passed or not; its check
//! before the change runs it in a worktree of its own made at that commit, taken away once it has run, and takes away
//! what such a check of the work left before.

use std::path::Path;

use super::{Branch, Host, InUse, Verdict};
use crate::git::{self, Before};
use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The project's local repository, as its own host.
#[derive(Clone, Debug)]
pub struct Local {
    project: Project,
    /// The variables of the engine's environment the check command is given beside those always given.
    env: Vec<String>,
    /// The worktrees of the checks before the change running.
    in_use: InUse,
}

impl Local {
    pub fn new(project: Project) -> Local {
        Local {
            project,
            env: Vec::new(),
            in_use: InUse::default(),
        }
    }

    /// The host whose check command is given these variables of the engine's environment, as the agents are.
    pub fn giving(mut self, env: Vec<String>) -> Local {
        self.env = env;
        self
    }

    /// Runs the check command in the directory, and says whether it passed with the end of what it said, `on` saying
    /// where after "passed" and "failed"; none where it was told to stop.
    async fn run(&self, dir: &Path, on: &str, stop: &mut Stop) -> Result<Option<String>, String> {
        let check = &self.project.check;
        let ran = process::run(process::shell(check, dir, &self.env), None, None, stop).await?;
        let Ran::Ended { status, stdout, .. } = ran else {
            return Ok(None);
        };
        let passed = if status.success() {
            "passed".to_string()
        } else {
            format!("failed ({status})")
        };
        let tail = process::tail(&stdout);
        Ok(Some(format!(
            "The check `{check}` {passed}{on}. The end of what it said:\n{tail}"
        )))
    }
}

impl Host for Local {
    async fn main(&self) -> Result<String, String> {
        git::git(&self.project.repo, &["rev-parse", "--verify", &self.kept_main()]).await
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

    fn kept_main(&self) -> String {
        format!("refs/heads/{}", self.project.main)
    }

    async fn check(&self, branch: &Branch<'_>, stop: &mut Stop) -> Result<Option<String>, String> {
        let ran = match git::worktree(branch.place) {
            Ok(dir) => self.run(&dir, "", stop).await,
            Err(why) => Err(why),
        };
        // What could not be run is said with the command, as what ran is.
        let check = &self.project.check;
        Ok(ran.unwrap_or_else(|why| Some(format!("The check `{check}` could not be run: {why}"))))
    }

    async fn check_before(
        &self,
        branch: &Branch<'_>,
        before: &Before,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        let mine = git::before_dir(branch.place, &before.commit).display().to_string();
        let _held = self.in_use.hold(&mine).await;
        // What no check of this process holds was left by one that ended.
        for left in git::befores(branch.place) {
            if !self.in_use.held(&left.display().to_string()) {
                git::take_away(&self.project, &left).await?;
            }
        }
        let dir = git::check_out_before(&self.project, branch.place, &before.commit).await?;
        let ran = self.run(&dir, " on it", stop).await;
        let taken = git::take_away(&self.project, &dir).await;
        let said = ran?;
        taken?;
        Ok(said)
    }
}
