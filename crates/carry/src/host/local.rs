//! The local repository as the host: main is its own main branch, the verdict is the project's check command run in
//! the work's worktree, and the merge is made in its checkout. An agent's `run_tests` runs the same command on the
//! worktree as it stands, what is not committed included, and says what it said, whether it passed or not.

use std::path::Path;

use flow::agents::{LocalWords, Tool, fill};

use super::{Branch, Host, Named, NotMerged, Verdict};
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
    /// What `run_tests` says here, from the tool's file beside the flows.
    words: Option<LocalWords>,
}

impl Local {
    pub fn new(project: Project) -> Local {
        Local {
            project,
            env: Vec::new(),
            words: None,
        }
    }

    /// The host that says what `run_tests` says in the words of the tool's file.
    pub fn saying(mut self, tool: Option<&Tool>) -> Local {
        self.words = tool.and_then(|tool| tool.local.clone());
        self
    }

    /// The host whose check command is given these variables of the engine's environment, as the agents are.
    pub fn giving(mut self, env: Vec<String>) -> Local {
        self.env = env;
        self
    }

    /// What `run_tests` says here, or why there are no words.
    fn words(&self) -> Result<&LocalWords, String> {
        (self.words.as_ref())
            .ok_or_else(|| "the words of run_tests (agents/tools/run_tests.toml) have no [local]".to_string())
    }

    /// Runs the check command in the directory, and says whether it passed with what it said; none where it was told
    /// to stop.
    async fn run(&self, dir: &Path, stop: &mut Stop) -> Result<Option<String>, String> {
        let words = self.words()?;
        let check = &self.project.check;
        let ran = process::run(process::shell(check, dir, &self.env), None, None, stop).await?;
        let Ran::Ended { status, stdout, .. } = ran else {
            return Ok(None);
        };
        let outcome = if status.success() {
            words.passed.clone()
        } else {
            fill(&words.failed, &[("status", &status.to_string())])
        };
        let said = String::from_utf8_lossy(&stdout);
        Ok(Some(fill(
            &words.ran,
            &[("command", check), ("outcome", &outcome), ("said", &said)],
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
        Ok(Some(Verdict {
            failed,
            by: None,
            notes: Vec::new(),
        }))
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        git::merge(&self.project, branch.place, checked).await
    }

    fn kept_main(&self) -> String {
        kept_main(&self.project.main)
    }

    async fn run_tests(&self, branch: &Branch<'_>, named: &Named, stop: &mut Stop) -> Result<Option<String>, String> {
        let words = self.words()?;
        let ran = match git::worktree(branch.place) {
            Ok(dir) => self.run(&dir, stop).await,
            Err(why) => Err(why),
        };
        // What could not be run is said with the command, as what ran is.
        let check = &self.project.check;
        let said = ran.unwrap_or_else(|why| Some(fill(&words.could_not, &[("command", check), ("why", &why)])));
        // The tests and checks named are not run apart: the whole command runs.
        Ok(said.map(|said| {
            if named.is_empty() {
                said
            } else {
                format!("{}\n\n{said}", words.not_apart)
            }
        }))
    }

    /// The local repository holds nothing for a work beside its branch and its worktrees, which the acts let go.
    async fn end(&self, _: &Branch<'_>, _: bool) -> Result<(), String> {
        Ok(())
    }
}

/// The reference that holds main's head as the local repository gives it, main of this name: the branch itself.
pub(crate) fn kept_main(main: &str) -> String {
    format!("refs/heads/{main}")
}
