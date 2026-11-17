//! A stand-in for the agents' command line, with which `wf-agents cases build` builds what each case's cell is sent
//! through the engine, its texts and their hashes, with no model (design document `testing`, 16). It takes the message
//! a round is told on its standard input, says it took it in, as the command line's stream does as it begins, and
//! returns what the case's carrier wrote for the round in `stand-in.json` of the installation's data directory, which
//! holds the worktree it runs in: the round deals with what ran it and hands in nothing. It reads none of the
//! arguments the engine gives the command line.

use std::io::BufRead;
use std::process::ExitCode;

/// The file the case's carrier writes the return in, in the installation's data directory.
const RETURN: &str = "stand-in.json";

fn main() -> ExitCode {
    let mut message = String::new();
    if let Err(e) = std::io::stdin().lock().read_line(&mut message) {
        eprintln!("the message could not be read: {e}");
        return ExitCode::FAILURE;
    }
    // The worktree is `worktrees/<name>` of the data directory.
    let data = std::env::current_dir()
        .ok()
        .and_then(|dir| dir.parent()?.parent().map(|d| d.to_path_buf()));
    let back = data.and_then(|data| std::fs::read_to_string(data.join(RETURN)).ok());
    let Some(back) = back else {
        eprintln!("{RETURN} could not be read beside the worktree");
        return ExitCode::FAILURE;
    };
    println!(r#"{{"type":"system","subtype":"init","claude_code_version":"0.0.0"}}"#);
    let usage = r#"{"input_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":1}"#;
    println!(
        r#"{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0,"usage":{usage},"structured_output":{}}}"#,
        back.trim()
    );
    ExitCode::SUCCESS
}
