//! The local repository as the host: main is its own main branch, the verdict is the project's check command run on a
//! checkout of the commit to merge and of nothing else, taken away as the command ends, and the merge is made in the
//! repository's checkout. An agent's `run_tests` runs the same command on the worktree as it stands, what is not
//! committed included, and says what it said, whether it passed or not.

use std::path::Path;

use flow::agents::{LocalWords, Tool, fill};
use tokio::sync::Mutex;

use super::{Asking, Branch, Failed, Host, Named, NotMerged, Verdict};
use crate::git;
use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The project's local repository, as its own host.
#[derive(Clone, Debug)]
pub struct Local {
    project: Project,
    /// The shell command whose exit with 0 means the work passes, from the project's declaration (`[local] check`).
    check: String,
    /// The variables of the engine's environment the check command is given beside those always given.
    env: Vec<String>,
    /// What `run_tests` says here, from the tool's file beside the flows.
    words: Option<LocalWords>,
}

impl Local {
    pub fn new(project: Project, check: String) -> Local {
        Local {
            project,
            check,
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
        let check = &self.check;
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

    /// The command runs on a checkout of the commit alone, which the verdict is on and which is what a merge would
    /// land: what the work's worktree holds besides it, a file left there and not committed or an ignored cache, is no
    /// part of what is judged. The checkout is taken away when the command ends, or is stopped. The checkout is made
    /// and taken away holding the repository, which registers and drops a worktree there as the acts' own do; the
    /// command runs without it.
    async fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        _: &str,
        _: Asking,
        repository: &Mutex<()>,
        stop: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        let check = &self.check;
        let dir = {
            let _held = repository.lock().await;
            git::check_out(&self.project, branch.place, commit).await?
        };
        let ran = process::run(process::shell(check, &dir, &self.env), None, None, stop).await;
        let taken = {
            let _held = repository.lock().await;
            git::take_away_check(&self.project, branch.place).await
        }
        .map_err(|why| format!("the checkout the check ran on could not be taken away: {why}"));
        let failed = match ran? {
            Ran::Ended { status, stdout, .. } if !status.success() => Some(format!(
                "The check `{check}` failed ({status}). The end of what it said:\n{}",
                process::tail(&stdout)
            )),
            Ran::Ended { .. } => None,
            Ran::Stopped => {
                taken?;
                return Ok(None);
            }
        };
        taken?;
        Ok(Some(Verdict {
            failed,
            ..Verdict::default()
        }))
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        git::merge(&self.project, branch.place, branch.title, checked).await
    }

    fn kept_main(&self) -> String {
        kept_main(&self.project.main)
    }

    async fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        _: Asking,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        let words = self.words()?;
        // The command runs on the worktree as it is, so a commit named is no commit that could be run.
        if named.commit.is_some() {
            return Ok(Some(words.no_commit.clone()));
        }
        let ran = match git::worktree(branch.place) {
            Ok(dir) => self.run(&dir, stop).await,
            Err(why) => Err(why),
        };
        // What could not be run is said with the command, as what ran is.
        let check = &self.check;
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

    /// A local host holds nothing for a work but its branch, which the work keeps.
    async fn reopen(&self, _: &Branch<'_>) -> Result<(), String> {
        Ok(())
    }

    /// The local repository has no pull requests: a work merges in its checkout.
    fn pull_at(&self, _: u64) -> Option<String> {
        None
    }
}

/// The reference that holds main's head as the local repository gives it, main of this name: the branch itself.
pub(crate) fn kept_main(main: &str) -> String {
    format!("refs/heads/{main}")
}
