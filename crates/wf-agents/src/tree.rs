//! The design tree check: what the design tree holds now and how much more than at the fork point from main, and
//! whether each node changed since the fork point is in the node format the flow's rule of design tree writing sets. A
//! project names it as its `design_check`, run by the engine in a work's worktree with the fork point as its last
//! argument: the design tree review is given what it prints, and a round whose node is not in the format is sent back
//! with its faults.

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

/// Each fault of a node's text against the node format, with the number of its line: one or more `Decision:` lines,
/// optionally followed by a `Rejected:` list; each field one line, with a blank line between fields, and the list's
/// items straight after its heading or the item before; a decision gives its reason after `because`, the approach not
/// chosen after `instead of`, or both; each rejected item reads `- <approach>: rejected because <reason>`; and nothing
/// but ASCII.
pub fn faults(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut decisions = 0;
    let mut rejected = false;
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        let before = if i == 0 { None } else { Some(lines[i - 1]) };
        let mut fault = |what: &str| out.push((n, what.to_string()));
        if !line.is_ascii() {
            fault("holds a character that is not ASCII");
        }
        if line.is_empty() {
            if before.is_none_or(str::is_empty) {
                fault("a blank line where a field must be");
            }
            continue;
        }
        let field = line.starts_with("Decision:") || line.starts_with("Rejected:");
        if field && before.is_some_and(|b| !b.is_empty()) {
            fault("a field with no blank line before it");
        }
        if line.starts_with("Decision:") {
            let lower = line.to_ascii_lowercase();
            if !lower.contains("because") && !lower.contains("instead of") {
                fault(
                    "a decision with neither its reason after `because` nor the approach not chosen after `instead of`",
                );
            }
            if rejected {
                fault("a decision after the `Rejected:` list");
            }
            decisions += 1;
        } else if line.starts_with("Rejected:") {
            if *line != "Rejected:" {
                fault("the `Rejected:` heading holds more than itself");
            }
            if rejected {
                fault("a second `Rejected:` list");
            }
            rejected = true;
        } else if line.starts_with("- ") {
            if before.is_none_or(|b| b != "Rejected:" && !b.starts_with("- ")) {
                fault("a list item not straight after the `Rejected:` heading or the item before");
            }
            if !line.contains(": rejected because ") {
                fault("a rejected item that does not read `- <approach>: rejected because <reason>`");
            }
        } else {
            fault("neither a field (`Decision:` or `Rejected:`), a list item nor a blank line");
        }
    }
    if decisions == 0 {
        out.push((1, "no `Decision:` line".to_string()));
    }
    match lines.last() {
        Some(&"") => out.push((lines.len(), "a blank line at the end".to_string())),
        Some(&"Rejected:") => out.push((lines.len(), "a `Rejected:` list with no item".to_string())),
        _ => {}
    }
    out
}

/// Each fault of the nodes under `root` added or changed since the fork point, as they are at HEAD, each as
/// `<path>:<line>: <what>`. Nodes not changed are left as they are: the format holds a node from its next change.
pub fn node_faults(dir: &Path, fork: &str, root: &str) -> Result<Vec<String>, String> {
    let root = root.trim_end_matches('/');
    let changed = git(
        dir,
        &["diff", "--name-only", "--diff-filter=d", fork, "HEAD", "--", root],
    )?;
    let mut out = Vec::new();
    for f in changed.lines().filter(|f| f.ends_with(".md")) {
        let text = git(dir, &["show", &format!("HEAD:{f}")])?;
        out.extend(faults(&text).into_iter().map(|(n, what)| format!("{f}:{n}: {what}")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A node in the format has no fault; each way out of it is found on its line.
    #[test]
    fn a_node_out_of_the_format_is_found_by_its_lines() {
        let good = "Decision: One table, because two drift.\n\nDecision: No cache, instead of one.\n\nRejected:\n- Two \
                    tables: rejected because they drift apart.\n- A cache: rejected because nothing is slow.\n";
        assert_eq!(faults(good), Vec::<(usize, String)>::new());
        let lines = |text: &str| faults(text).into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        // A decision with no reason, and one straight after another with no blank line.
        assert_eq!(lines("Decision: One table.\nDecision: Two, because three.\n"), [1, 2]);
        // An item not written as rejected, a list item after a blank line, and a decision after the list.
        assert_eq!(
            lines(
                "Decision: A, because b.\n\nRejected:\n- C: no.\n\n- D: rejected because e.\n\nDecision: F, because g.\n"
            ),
            [4, 6, 8]
        );
        // A character that is not ASCII, a line that is no field, two blank lines and one at the end.
        assert_eq!(
            lines("Decision: A \u{2192} B, because c.\nNotes.\n\n\nDecision: D, because e.\n\n"),
            [1, 2, 4, 6]
        );
        // A list with no item and no decision.
        assert_eq!(lines("Rejected:\n"), [1, 1]);
    }

    // Only the nodes changed since the fork point are held to the format, as HEAD has them.
    #[test]
    fn only_the_nodes_changed_since_the_fork_point_are_checked() {
        let dir = std::env::temp_dir().join(format!("wf-agents-tree-faults-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("design/engine")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(dir.join("design/engine.md"), "Notes, before the format.\n").unwrap();
        std::fs::write(dir.join("design/flow.md"), "Decision: One, because two.\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "fork"]);
        let fork = run(&["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(dir.join("design/engine/tools.md"), "Decision: Three.\n").unwrap();
        std::fs::write(
            dir.join("design/flow.md"),
            "Decision: One, because two.\n\nDecision: Four, because five.\n",
        )
        .unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "more"]);
        assert_eq!(
            node_faults(&dir, &fork, "design/").unwrap(),
            [
                "design/engine/tools.md:1: a decision with neither its reason after `because` nor the approach not chosen \
              after `instead of`"
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

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
