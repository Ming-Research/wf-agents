//! What the program that runs the cases asks beside the cases' own rounds (design document `testing`, 14, 15, 34): the
//! words of a fork of a round's session about the cell's text, which the agents' carrier asks through the round's own
//! command line ([`carry::Agents::fork`]); and an agent of its own, in a session of its own, that judges what only
//! reading can judge of a return, run as the agents' command line the installation names, with the environment a
//! round's command line is given ([`carry::cli::environment`]), on the model the case's cell runs on, with no tool.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

/// What the program asks beside the cases, as `words.toml` beside the cases says it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Words {
    pub fork: String,
    /// What a fork is told where it would use a tool, which it may not.
    pub fork_refused: String,
    pub judge: String,
    pub judge_form: toml::Value,
}

/// The file beside the cases that says what is asked.
pub const WORDS: &str = "words.toml";

/// Reads what is asked, from the directory that holds the flows' cases.
pub fn words(dir: &Path) -> Result<Words, String> {
    let path = dir.join(WORDS);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))
}

/// What the agents' program answered, and what it cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub text: String,
    pub value: Option<serde_json::Value>,
    pub cost_usd: f64,
}

/// Runs the agents' program with these arguments in this directory, the prompt on its standard input, and reads its
/// one result.
fn ask(settings: &carry::Settings, dir: &Path, args: &[String], prompt: &str) -> Result<Answer, String> {
    let env = carry::cli::environment(&settings.agents, std::env::vars());
    let mut child = Command::new(&settings.agents.command)
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", settings.agents.command))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("the prompt could not be given: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("{}: {e}", settings.agents.command))?;
    let read: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        let said = String::from_utf8_lossy(&out.stderr);
        format!("it gave no result: {}", said.trim())
    })?;
    Ok(Answer {
        text: read
            .get("result")
            .and_then(|r| r.as_str())
            .unwrap_or_default()
            .to_string(),
        value: read.get("structured_output").cloned(),
        cost_usd: read.get("total_cost_usd").and_then(|c| c.as_f64()).unwrap_or_default(),
    })
}

/// The arguments every call here is given: one prompt, its result as JSON, no tool, no setting but the user's, no
/// server, no command of its own and no memory.
fn plain(model: &str) -> Vec<String> {
    let settings = serde_json::json!({ "autoMemoryEnabled": false }).to_string();
    [
        "-p",
        "--output-format",
        "json",
        "--model",
        model,
        "--tools",
        "",
        "--setting-sources",
        "user",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--settings",
        settings.as_str(),
    ]
    .map(str::to_string)
    .to_vec()
}

/// Has an agent judge what only reading can judge of the return: whether it passes, and why.
pub fn judge(
    settings: &carry::Settings,
    words: &Words,
    dir: &Path,
    model: &str,
    asked: (&str, &str, &str),
) -> Result<(bool, String, f64), String> {
    let (judge, message, back) = asked;
    let prompt = flow::agents::fill(
        words.judge.trim(),
        &[("judge", judge), ("message", message), ("back", back)],
    );
    let form = serde_json::to_string(&words.judge_form).map_err(|e| e.to_string())?;
    let mut args = plain(model);
    args.extend(["--json-schema".to_string(), form]);
    let answer = ask(settings, dir, &args, &prompt)?;
    let value = answer.value.ok_or("the judge returned no judgment")?;
    let passed = value
        .get("passed")
        .and_then(|p| p.as_bool())
        .ok_or("the judgment says no pass")?;
    let why = value
        .get("why")
        .and_then(|w| w.as_str())
        .unwrap_or_default()
        .to_string();
    Ok((passed, why, answer.cost_usd))
}

/// The agents' program's version, as it says it; `unknown` where it says none.
pub fn version(settings: &carry::Settings) -> String {
    let out = Command::new(&settings.agents.command)
        .arg("--version")
        .stdin(Stdio::null())
        .output();
    match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}
