//! What the repository's own flow files say, read as plain text: they name no agents' program, and the two files of
//! what every agent may do and is refused hold no tool, flag or path of any program, since the flow is independent of
//! the program that runs its agents (constitution Article 1; the program's own names are the engine's adapter's).

use std::fs;
use std::path::{Path, PathBuf};

/// The repository's `flow/` directory.
fn flow_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// Every file under the directory, however deep.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a directory of the flows is read") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

// No file of the repository's flows names the program that runs the agents (`claude`, in any case), whichever its kind:
// a name there would make the flow, and so every installation, assume one program. Reference: constitution Article 1
// ("independent of any coding agent") and the work's acceptance 3 and 8. Milliseconds; the one test that reads every
// flow file for a program's name, which the typed reads of the files cannot see in a comment or a text.
#[test]
fn no_file_of_the_flows_names_the_agents_program() {
    let mut all = Vec::new();
    files(&flow_root(), &mut all);
    assert!(all.len() > 20, "the flows' files were found: {}", all.len());
    for path in all {
        let Ok(text) = fs::read_to_string(&path) else { continue };
        assert!(
            !text.to_lowercase().contains("claude"),
            "{} names the agents' program",
            path.display()
        );
    }
}

// What every agent may do and is refused, in `allowed.toml` and `refused.toml`, holds no tool name, flag, rule syntax or
// path of a program, in its entries or its comments: the words are the flow's and the program's own names are written
// only where the engine turns them. Reference: the work's acceptance 3 ("names no program, tool, rule syntax or path")
// and the program's own names for these (its tools reference and command-line reference). Milliseconds.
#[test]
fn what_agents_may_do_and_are_refused_is_written_without_a_programs_tools_flags_rules_or_paths() {
    for file in ["agents/allowed.toml", "agents/refused.toml"] {
        let text = fs::read_to_string(flow_root().join(file)).expect("the file is read");
        for token in [
            "Bash",
            "Edit(",
            "Read(",
            "Write(",
            "Grep",
            "Glob",
            "WebSearch",
            "WebFetch",
            "Task",
            "Agent",
            "--tools",
            "--allowedTools",
            "--disallowedTools",
            "--add-dir",
            "TMPDIR",
            "/tmp",
            "/var",
            "/Users",
            "/home",
        ] {
            assert!(!text.contains(token), "{file} holds {token:?}");
        }
    }
}
