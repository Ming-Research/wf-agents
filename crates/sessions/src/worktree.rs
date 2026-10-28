//! Where a work's agents work: the session manager sets up a work's worktree and branch on a machine with a free slot,
//! says whether the head a round handed in is the worktree's and nothing is left uncommitted, and removes the worktree
//! once the work has ended. Worktrees are caches that can be made again: the database and the branch on GitHub are the
//! state.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use db::rusqlite::{Connection, params};

/// Runs git in `dir` and gives its output, or what it said on failure.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() {
            String::from_utf8_lossy(&out.stdout).into_owned()
        } else {
            err.into_owned()
        };
        return Err(format!("git {}: {}", args.join(" "), msg.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Fetches main and makes a worktree on a new branch from it; gives the commit it starts from.
pub fn create_worktree(checkout: &Path, dir: &Path, branch: &str, main: &str) -> Result<String, String> {
    git(checkout, &["fetch", "-q", "origin", main])?;
    let base = format!("origin/{main}");
    let d = dir.to_string_lossy();
    git(checkout, &["worktree", "add", "-q", "-b", branch, &d, &base])?;
    head(dir)
}

/// Takes over a branch: the worktree when it is there, otherwise one made afresh from the branch, the local one or else
/// the remote one fetched; gives the commit it stands at.
pub fn take_over_worktree(checkout: &Path, dir: &Path, branch: &str) -> Result<String, String> {
    if dir.join(".git").exists() {
        return head(dir);
    }
    // A worktree removed by hand is still registered until pruned, and its branch cannot be checked out again.
    git(checkout, &["worktree", "prune"])?;
    let d = dir.to_string_lossy();
    if resolve(checkout, &format!("refs/heads/{branch}")).is_some() {
        git(checkout, &["worktree", "add", "-q", &d, branch])?;
    } else {
        let remote = format!("refs/remotes/origin/{branch}");
        git(
            checkout,
            &["fetch", "-q", "origin", &format!("refs/heads/{branch}:{remote}")],
        )?;
        git(checkout, &["worktree", "add", "-q", "-b", branch, &d, &remote])?;
    }
    head(dir)
}

/// Removes a work's worktree; its branch stays.
pub fn remove_worktree(checkout: &Path, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        git(checkout, &["worktree", "remove", "--force", &dir.to_string_lossy()])?;
    }
    Ok(())
}

pub fn head(dir: &Path) -> Result<String, String> {
    Ok(git(dir, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// The full id of a commit, or none when there is no such commit.
pub fn resolve(dir: &Path, rev: &str) -> Option<String> {
    git(dir, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
        .ok()
        .map(|s| s.trim().to_string())
}

/// Whether the worktree has nothing uncommitted, untracked files included.
pub fn is_clean(dir: &Path) -> Result<bool, String> {
    Ok(git(dir, &["status", "--porcelain"])?.trim().is_empty())
}

/// Why a head a round handed in does not stand for the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unfit {
    /// The worktree has no such commit.
    NoCommit,
    /// The worktree's HEAD is another commit, this one.
    NotHead(String),
    /// The worktree has changes not committed.
    Uncommitted,
    /// Git could not say.
    Git(String),
}

/// The commit a head handed in names, when it is the worktree's HEAD and nothing is left uncommitted.
pub fn handed_in(wt: &Path, head_given: &str) -> Result<String, Unfit> {
    let sha = resolve(wt, head_given).ok_or(Unfit::NoCommit)?;
    let local = head(wt).map_err(Unfit::Git)?;
    if sha != local {
        return Err(Unfit::NotHead(local));
    }
    if !is_clean(wt).map_err(Unfit::Git)? {
        return Err(Unfit::Uncommitted);
    }
    Ok(sha)
}

/// Where a work runs once the session manager has set it up: the machine, the worktree and its branch, the commit it
/// stands at, the pull request it takes over, if any, and the id the engineer's session will have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub machine: String,
    pub worktree: PathBuf,
    pub branch: String,
    pub head: String,
    pub pr: Option<i64>,
    pub session: String,
}

/// What an item brings to its new work: its id and project, and the branch and pull request an import recorded.
pub struct ItemPlace<'a> {
    pub id: i64,
    pub project: &'a str,
    pub imported_branch: Option<&'a str>,
    pub imported_pr: Option<i64>,
}

/// Sets up where a work runs, on the machine with a free slot the configuration names first (the first machine when
/// none is free): an item worked on here before takes over its latest earlier work's branch, worktree (reused when it
/// is there, made afresh from the branch otherwise) and pull request; an item imported with a branch takes that branch
/// and the pull request imported with it; any other gets a new branch from main. `free` is each machine's free slots.
pub fn set_up(
    conn: &Connection,
    data: &Path,
    project: &config::Project,
    item: &ItemPlace<'_>,
    work: i64,
    free: &BTreeMap<String, i64>,
) -> Result<Place, String> {
    let machine = free
        .iter()
        .find(|(_, n)| **n > 0)
        .or_else(|| free.iter().next())
        .map(|(m, _)| m.clone())
        .ok_or("the configuration names no machine")?;
    let fresh = config::worktrees(data).join(format!("{}-{work}", item.project));
    if let Some(parent) = fresh.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let earlier: Option<(Option<String>, String, Option<i64>)> = conn
        .query_row(
            "SELECT worktree, branch, pr FROM works
             WHERE item = ?1 AND id <> ?2 AND branch IS NOT NULL ORDER BY id DESC LIMIT 1",
            params![item.id, work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let (worktree, branch, pr, head) = match (earlier, item.imported_branch) {
        (Some((worktree, branch, pr)), _) => {
            let dir = worktree
                .map(PathBuf::from)
                .filter(|d| d.join(".git").exists())
                .unwrap_or(fresh);
            let head = take_over_worktree(&project.checkout, &dir, &branch)?;
            (dir, branch, pr, head)
        }
        (None, Some(branch)) => {
            let head = take_over_worktree(&project.checkout, &fresh, branch)?;
            (fresh, branch.to_string(), item.imported_pr, head)
        }
        (None, None) => {
            let branch = format!("wf-agents/{}-{work}", item.id);
            let head = create_worktree(&project.checkout, &fresh, &branch, &project.main)?;
            (fresh, branch, None, head)
        }
    };
    Ok(Place {
        machine,
        worktree,
        branch,
        head,
        pr,
        session: crate::cli::new_session_id(),
    })
}

/// Each machine's free slots: its slots less the works holding one there, a work on no machine yet holding one on the
/// first machine.
pub fn free_slots(hosts: &BTreeMap<String, config::Host>, holding: &[Option<String>]) -> BTreeMap<String, i64> {
    let first = hosts.keys().next().cloned();
    hosts
        .iter()
        .map(|(name, h)| {
            let held = holding
                .iter()
                .filter(|m| m.as_ref().or(first.as_ref()) == Some(name))
                .count() as i64;
            (name.clone(), i64::from(h.slots) - held)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each machine reports its slots less the works holding one there; a work not placed yet holds one on the first.
    #[test]
    fn each_machine_reports_its_free_slots() {
        let hosts = BTreeMap::from([
            ("a".to_string(), config::Host { slots: 2 }),
            ("b".to_string(), config::Host { slots: 1 }),
        ]);
        let free = free_slots(&hosts, &[Some("b".into()), None]);
        assert_eq!(free, BTreeMap::from([("a".to_string(), 1), ("b".to_string(), 0)]));
    }

    // A head stands for the worktree only when it is HEAD and nothing is left uncommitted.
    #[test]
    fn a_head_handed_in_must_be_the_clean_worktrees() {
        let dir = std::env::temp_dir().join(format!("wf-agents-sessions-head-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "{args:?}");
        };
        run(&["init", "-q"]);
        std::fs::write(dir.join("a"), "1").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "one"]);
        let first = head(&dir).unwrap();
        std::fs::write(dir.join("a"), "2").unwrap();
        run(&["commit", "-q", "-am", "two"]);
        let second = head(&dir).unwrap();
        assert_eq!(handed_in(&dir, &second), Ok(second.clone()));
        assert_eq!(handed_in(&dir, &first), Err(Unfit::NotHead(second.clone())));
        assert_eq!(handed_in(&dir, "0123456789abcdef"), Err(Unfit::NoCommit));
        std::fs::write(dir.join("b"), "new").unwrap();
        assert_eq!(handed_in(&dir, &second), Err(Unfit::Uncommitted));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
