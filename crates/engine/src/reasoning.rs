//! The reasoning system a project that keeps records keeps them by: the installation's one copy of it, whose rules the
//! cells that work on the records are given by role, and whose check the engine runs on the records (cell `D6`) and
//! the cells run themselves.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::Engine;

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
}
