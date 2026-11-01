//! The reasoning system a project that keeps records keeps them by: the installation's one copy of it, whose rules the
//! cells that work on the records are given by role, and whose check the engine runs on the records (cell `D6`) and
//! the cells run themselves; and the records as a commit has them, each by its reference, which a decision card's
//! revision is checked against.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::Engine;

/// A record the branch revised: whether it was added, changed or removed, its text at the head (at the fork point for
/// one removed) and, for one changed, its text at the fork point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revised {
    pub change: &'static str,
    pub text: String,
    pub before: Option<String>,
}

impl Revised {
    /// The record as a decision card's revision keeps it.
    pub fn json(&self, reference: &str) -> Value {
        let mut v = json!({ "reference": reference, "change": self.change, "text": self.text });
        if let Some(b) = &self.before {
            v["before"] = json!(b);
        }
        v
    }
}

/// Whether a word is a record's id: lowercase letters and digits, words joined by single hyphens.
pub(crate) fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.split('-')
            .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// Whether a text is a reference to a record: `C:`, `F:` or `D:` and an id.
pub fn is_reference(s: &str) -> bool {
    matches!(s.split_once(':'), Some(("C" | "F" | "D", id)) if is_id(id))
}

/// The id a line opens a fact or a derivation with, as the reasoning system writes them: `<word> <id>: <text>`.
fn head(line: &str, word: &str) -> Option<String> {
    let rest = line.strip_prefix(word)?.strip_prefix(' ')?;
    let (id, text) = rest.split_once(": ")?;
    (is_id(id) && text.starts_with(|c: char| !c.is_whitespace())).then(|| id.to_string())
}

/// The id an article of the constitution is headed with, `## Article <n> (<id>). <title>`.
fn article(line: &str) -> Option<String> {
    let rest = line.strip_prefix("## Article ")?;
    let (n, rest) = rest.split_once(" (")?;
    let (id, title) = rest.split_once("). ")?;
    (n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() && is_id(id) && !title.trim().is_empty())
        .then(|| id.to_string())
}

/// The records of one file of the records, by its path under their directory: the constitution's articles, or the
/// facts or derivations of a node file, each its text as the reasoning system's check compares it (an article's
/// number left out, since it is its place, not its content).
fn parse(path: &str, text: &str, out: &mut BTreeMap<String, String>) {
    let kind = if path == "constitution.md" {
        "C"
    } else if path.starts_with("facts/") {
        "F"
    } else if path.starts_with("derivations/") {
        "D"
    } else {
        return;
    };
    let mut current: Option<(String, Vec<String>)> = None;
    let close = |c: Option<(String, Vec<String>)>, out: &mut BTreeMap<String, String>| {
        if let Some((reference, mut lines)) = c {
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            out.entry(reference).or_insert_with(|| lines.join("\n"));
        }
    };
    for line in text.split('\n') {
        if kind == "C" && line.starts_with("## ") {
            close(current.take(), out);
            if let Some(id) = article(line) {
                // An article's number is its place, not its content.
                let rest = line
                    .trim_start_matches("## Article ")
                    .trim_start_matches(|c: char| c.is_ascii_digit());
                current = Some((format!("C:{id}"), vec![format!("## Article{rest}")]));
            }
            continue;
        }
        let opened = match kind {
            "F" => head(line, "Fact"),
            "D" => head(line, "Decision"),
            _ => None,
        };
        if let Some(id) = opened {
            close(current.take(), out);
            current = Some((format!("{kind}:{id}"), vec![line.to_string()]));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line.to_string());
        }
    }
    close(current, out);
}

/// The records of a project as the commit `commit` of the repository at `repo` has them under their directory `root`,
/// each its text by its reference.
pub fn records_at(repo: &Path, commit: &str, root: &str) -> Result<BTreeMap<String, String>, String> {
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let mut out = BTreeMap::new();
    for (path, text) in config::files_at(repo, commit, root)? {
        parse(path.strip_prefix(&prefix).unwrap_or(&path), &text, &mut out);
    }
    Ok(out)
}

/// What changed in a project's records from the commit `from` to the commit `to`: each record added, changed or removed,
/// by its reference.
pub fn revised(repo: &Path, from: &str, to: &str, root: &str) -> Result<BTreeMap<String, Revised>, String> {
    let before = records_at(repo, from, root)?;
    let after = records_at(repo, to, root)?;
    let mut out = BTreeMap::new();
    for (r, text) in &after {
        match before.get(r) {
            None => {
                out.insert(
                    r.clone(),
                    Revised {
                        change: "added",
                        text: text.clone(),
                        before: None,
                    },
                );
            }
            Some(b) if b != text => {
                out.insert(
                    r.clone(),
                    Revised {
                        change: "changed",
                        text: text.clone(),
                        before: Some(b.clone()),
                    },
                );
            }
            Some(_) => {}
        }
    }
    for (r, text) in before {
        if !after.contains_key(&r) {
            out.insert(
                r,
                Revised {
                    change: "removed",
                    text,
                    before: None,
                },
            );
        }
    }
    Ok(out)
}

/// The engine's inputs that give a rule file of the reasoning system, each with its file under `rules/`.
pub const RULES: [(&str, &str); 5] = [
    ("reasoning_reading", "reading.md"),
    ("reasoning_writing", "writing.md"),
    ("reasoning_cards", "extension-cards.md"),
    ("reasoning_work", "extension-work.md"),
    ("reasoning_dcr", "dcr.md"),
];

/// The engine's input that gives the command a cell runs the check with.
pub const CHECK: &str = "reasoning_check";

/// Whether an engine input is one of the reasoning system's.
pub fn is_input(name: &str) -> bool {
    name == CHECK || RULES.iter().any(|(n, _)| *n == name)
}

/// Where the installation's copy of the reasoning system is.
pub fn system(e: &Engine) -> PathBuf {
    e.config.config.install.reasoning.system(&e.data)
}

/// What an input of the reasoning system's gives a cell of a project's work: a rule file's text, or the command that
/// runs the check on the project's records in the work's worktree; none for a project that keeps no records. A rule
/// the installation's copy lacks is an error, not an input left out: a cell that works on the records needs it.
pub fn input(e: &Engine, project: &config::Project, name: &str) -> Result<Option<Value>, String> {
    let Some(records) = &project.records else {
        return Ok(None);
    };
    let sys = system(e);
    if name == CHECK {
        let mut words = vec![
            "python3".to_string(),
            "-B".to_string(),
            check_py(&sys).display().to_string(),
        ];
        words.extend(options(records));
        return Ok(Some(Value::String(words.join(" "))));
    }
    let file = RULES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .ok_or_else(|| format!("no such input of the reasoning system: {name}"))?;
    let path = sys.join("rules").join(file);
    std::fs::read_to_string(&path)
        .map(|t| Some(Value::String(t)))
        .map_err(|err| format!("the reasoning system's rule {}: {err}", path.display()))
}

fn check_py(sys: &Path) -> PathBuf {
    sys.join("check.py")
}

/// What check.py is told of a project's records: where they are, and to hold them to English when they are kept so.
fn options(records: &config::Records) -> Vec<String> {
    let mut out = vec!["--root".to_string(), records.root.clone()];
    if records.ascii {
        out.push("--ascii".to_string());
    }
    out
}

/// Cell `D6`: the reasoning system's check on the project's records in the worktree `wt`, against the fork point
/// `base` and with `--ready`, so that every record the branch changed is named in a new entry of their log: green
/// when it passes, red when it fails, each with what it printed.
pub fn check(e: &Engine, project: &config::Project, wt: &Path, base: &str) -> Result<Value, String> {
    let records = project
        .records
        .as_ref()
        .ok_or("the project keeps no records, so there is nothing to check")?;
    let script = check_py(&system(e));
    let mut args = vec!["-B".to_string(), script.display().to_string()];
    args.extend(options(records));
    args.extend(["--base".to_string(), base.to_string(), "--ready".to_string()]);
    let out = Command::new("python3")
        .args(&args)
        .current_dir(wt)
        .output()
        .map_err(|err| format!("python3 {}: {err}", args.join(" ")))?;
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = if printed.trim().is_empty() {
        "It printed nothing.".to_string()
    } else {
        crate::hub::tail(&printed, 200)
    };
    let verdict = if out.status.success() { "green" } else { "red" };
    Ok(json!({ "check": { "verdict": verdict, "printed": printed } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // check.py is told where the records are, and holds them to English only when the project keeps them so.
    #[test]
    fn the_check_is_told_where_the_records_are() {
        let english = config::Records {
            root: "reasoning".into(),
            ascii: true,
        };
        assert_eq!(options(&english), ["--root", "reasoning", "--ascii"]);
        let any = config::Records {
            root: "docs/records".into(),
            ascii: false,
        };
        assert_eq!(options(&any), ["--root", "docs/records"]);
        assert!(is_input("reasoning_dcr") && is_input(CHECK) && !is_input("project_guide"));
    }

    // The records are read as the reasoning system's check reads them: each article, fact and derivation its text by
    // its reference, an article's number left out, a record running to the next one; and a reference is a kind's
    // letter and an id.
    #[test]
    fn the_records_are_read_by_their_references() {
        let mut out = BTreeMap::new();
        parse(
            "constitution.md",
            "# The constitution\n\n## Article 1 (independent). Independent\n\nText.\n\n## Article 2 (information). Info\n\nMore.\n",
            &mut out,
        );
        parse(
            "derivations/flow.md",
            "# Flow\n\nDecision one: A, because B.\n\nRests on: C:independent\n\n\nDecision two-b: C, because D.\n",
            &mut out,
        );
        parse("facts/owner.md", "# Owner\n\nFact rules: The owner rules.\n", &mut out);
        parse("log.md", "Decision not: a log entry is no record.\n", &mut out);
        assert_eq!(out["C:independent"], "## Article (independent). Independent\n\nText.");
        assert_eq!(out["D:one"], "Decision one: A, because B.\n\nRests on: C:independent");
        assert_eq!(out["D:two-b"], "Decision two-b: C, because D.");
        assert_eq!(out["F:rules"], "Fact rules: The owner rules.");
        assert_eq!(out.len(), 5, "{out:?}");
        assert!(is_reference("D:two-b") && is_reference("F:a1") && !is_reference("D:Two") && !is_reference("X:a"));
        assert!(!is_reference("D:") && !is_reference("D:a--b") && !is_reference("two-b"));
    }

    // What the branch revised is each record added, changed or removed between two commits, read with git alone.
    #[test]
    fn what_the_branch_revised_is_each_record_added_changed_or_removed() {
        let dir = std::env::temp_dir().join(format!("wf-agents-revised-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |path: &str, text: &str| {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        put(
            "r/derivations/a.md",
            "Decision kept: K.\n\nDecision changed: Old.\n\nDecision gone: G.\n",
        );
        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["commit", "-qm", "first"]);
        let first = run(&["rev-parse", "HEAD"]);
        put("r/derivations/a.md", "Decision kept: K.\n\nDecision changed: New.\n");
        put("r/facts/b.md", "Fact added: A.\n");
        put("other/c.md", "Decision outside: not a record.\n");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "second"]);
        let got = revised(&dir, &first, "HEAD", "r").unwrap();
        let changes: Vec<(&str, &str)> = got.iter().map(|(r, x)| (r.as_str(), x.change)).collect();
        assert_eq!(
            changes,
            [("D:changed", "changed"), ("D:gone", "removed"), ("F:added", "added")]
        );
        assert_eq!(got["D:changed"].text, "Decision changed: New.");
        assert_eq!(got["D:changed"].before.as_deref(), Some("Decision changed: Old."));
        assert_eq!(got["D:gone"].text, "Decision gone: G.");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
