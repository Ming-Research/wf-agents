//! The flow as it is at a git ref of a project's checkout: `flow/flow.json` and the `db/schema.sql` it is checked
//! against, read from that commit with git and compiled by the flow's compiler, so that a flow change on a branch can
//! be looked at and compared before it is merged. Nothing is written: the checkout is only read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};

use flow::Compiled;

use crate::Error;

type Result<T> = std::result::Result<T, Error>;

/// The longest ref the interface takes.
const REF_MAX: usize = 200;

/// How many of a checkout's branches are offered.
const REFS: &str = "40";

/// The flows compiled at a commit so far, by checkout and commit. A commit never changes, so each is compiled once.
#[derive(Default)]
pub struct Sources(Mutex<BTreeMap<(PathBuf, String), Arc<Compiled>>>);

impl Sources {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<(PathBuf, String), Arc<Compiled>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The commit a ref of the checkout names, and the flow compiled as it is there.
    pub fn at(&self, checkout: &Path, rev: &str) -> Result<(String, Arc<Compiled>)> {
        let commit = commit(checkout, rev)?;
        let key = (checkout.to_path_buf(), commit.clone());
        if let Some(c) = self.lock().get(&key) {
            return Ok((commit, Arc::clone(c)));
        }
        let c = Arc::new(compile_at(checkout, &commit)?);
        self.lock().insert(key, Arc::clone(&c));
        Ok((commit, c))
    }
}

fn git(checkout: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(args)
        .output()
        .map_err(|e| Error::Failed(format!("git in {}: {e}", checkout.display())))?;
    if !out.status.success() {
        return Err(Error::NotFound(format!(
            "git {} in {}: {}",
            args.join(" "),
            checkout.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    String::from_utf8(out.stdout).map_err(|e| Error::Failed(format!("git {}: {e}", args.join(" "))))
}

/// The commit a ref names. A ref is a branch, a tag, a commit id or an expression git reads as one, such as
/// `main~2`; it never starts with `-`, so that git cannot take it for an option.
pub fn commit(checkout: &Path, rev: &str) -> Result<String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || "._/-~^@{}".contains(c);
    if rev.is_empty() || rev.len() > REF_MAX || rev.starts_with('-') || !rev.chars().all(allowed) {
        return Err(Error::Invalid(format!("not a git ref: {rev:?}")));
    }
    let id = git(
        checkout,
        &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")],
    )
    .map_err(|_| Error::NotFound(format!("no commit {rev} in {}", checkout.display())))?;
    Ok(id.trim().to_string())
}

/// The flow at a commit of the checkout, compiled against the database schema at the same commit.
pub fn compile_at(checkout: &Path, commit: &str) -> Result<Compiled> {
    let text = git(checkout, &["show", &format!("{commit}:flow/flow.json")])?;
    let schema = git(checkout, &["show", &format!("{commit}:db/schema.sql")])?;
    let at = |e| Error::Invalid(format!("db/schema.sql at {commit}: {e}"));
    let relations = db::relations(&schema).map_err(at)?;
    let tables = db::index_of(&schema).map_err(at)?;
    flow::compile(&text, &flow::Context { relations, tables })
        .map_err(|e| Error::Invalid(format!("flow/flow.json at {commit} does not compile:\n{e}")))
}

/// The checkout's branches, local and remote, the most recently committed first.
pub fn refs(checkout: &Path) -> Result<Vec<String>> {
    let out = git(
        checkout,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            &format!("--count={REFS}"),
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.ends_with("/HEAD"))
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repository whose one commit holds this repository's flow and schema.
    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-source-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("flow")).unwrap();
        std::fs::create_dir_all(dir.join("db")).unwrap();
        std::fs::write(dir.join("flow/flow.json"), flow::FLOW).unwrap();
        std::fs::write(dir.join("db/schema.sql"), db::SCHEMA).unwrap();
        run(&dir, &["init", "--quiet", "--initial-branch=main"]);
        run(&dir, &["add", "."]);
        run(&dir, &["commit", "--quiet", "-m", "flow"]);
        dir
    }

    #[test]
    fn the_flow_at_a_ref_is_what_the_compiler_makes_of_it_there() {
        let dir = repo("compile");
        let sources = Sources::default();
        let (commit, c) = sources.at(&dir, "main").unwrap();
        assert_eq!(commit.len(), 40);
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let here = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        assert_eq!(c.digest(), here.digest());
        // Asked again, the same commit is not compiled again.
        let (_, again) = sources.at(&dir, "HEAD").unwrap();
        assert!(Arc::ptr_eq(&c, &again));
        assert_eq!(refs(&dir).unwrap(), vec!["main".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ref_that_is_no_commit_or_looks_like_an_option_is_refused() {
        let dir = repo("refused");
        assert!(matches!(commit(&dir, "--output=x"), Err(Error::Invalid(_))));
        assert!(matches!(commit(&dir, "main;ls"), Err(Error::Invalid(_))));
        assert!(matches!(commit(&dir, "no-such-branch"), Err(Error::NotFound(_))));
        // A commit without the flow is no flow.
        std::fs::remove_file(dir.join("flow/flow.json")).unwrap();
        run(&dir, &["commit", "--quiet", "-am", "no flow"]);
        assert!(compile_at(&dir, "HEAD").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
