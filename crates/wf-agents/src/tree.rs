//! The design tree check: what the design tree holds now and how much more than at the fork point from main. A
//! project names it as its `design_check`, run by the engine in a work's worktree with the fork point as its last
//! argument, and the design tree review is given what it prints.

use std::path::Path;
use std::process::Command;

/// What a design tree holds at a revision: its nodes (the Markdown files under its root), how deep they go below the
/// root, and its decisions (the paragraphs that start "Decision:").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tree {
    pub nodes: i64,
    pub depth: i64,
    pub decisions: i64,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The design tree under `root` as it is at `rev` of the repository at `dir`.
pub fn tree_at(dir: &Path, rev: &str, root: &str) -> Result<Tree, String> {
    let root = root.trim_end_matches('/');
    let files = git(dir, &["ls-tree", "-r", "--name-only", rev, "--", root])?;
    let mut t = Tree::default();
    for f in files.lines().filter(|f| f.ends_with(".md")) {
        t.nodes += 1;
        let below = f.strip_prefix(root).unwrap_or(f).trim_start_matches('/');
        t.depth = t.depth.max(below.split('/').count() as i64);
        let text = git(dir, &["show", &format!("{rev}:{f}")])?;
        t.decisions += text.lines().filter(|l| l.starts_with("Decision:")).count() as i64;
    }
    Ok(t)
}

/// What the check prints: each number at HEAD, with how many more than at the fork point.
pub fn report(dir: &Path, fork: &str, root: &str) -> Result<String, String> {
    let now = tree_at(dir, "HEAD", root)?;
    let then = tree_at(dir, fork, root)?;
    Ok(format!(
        "nodes: {} ({:+} since the fork point)\ndepth: {} ({:+})\ndecisions: {} ({:+})\n",
        now.nodes,
        now.nodes - then.nodes,
        now.depth,
        now.depth - then.depth,
        now.decisions,
        now.decisions - then.decisions
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The check counts the nodes, the depth and the decisions at HEAD, and how many more than at the fork point.
    #[test]
    fn the_design_tree_is_counted_against_the_fork_point() {
        let dir = std::env::temp_dir().join(format!("wf-agents-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("design/engine")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(dir.join("design/engine.md"), "Decision: one.\n\nDecision: two.\n").unwrap();
        std::fs::write(dir.join("README.md"), "Decision: not in the tree.\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "fork"]);
        let fork = run(&["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(dir.join("design/engine/tools.md"), "Decision: three.\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "more"]);
        assert_eq!(
            report(&dir, &fork, "design/").unwrap(),
            "nodes: 2 (+1 since the fork point)\ndepth: 2 (+1)\ndecisions: 3 (+1)\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
