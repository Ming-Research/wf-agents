//! What the agents of a line may do and run, as the command line is given it each round, and what the agent is told in
//! its system prompt that no rule says by itself, in the words beside the flows.
//!
//! What agents may do is said in the words of `agents/allowed.toml` beside the flows (shell commands, files in the
//! worktree, files in the temporary directory, the web, subagents), which name no program. This module is the adapter
//! for `claude`: it turns each word into the tools, the allow rules and the directories the program's own flags take
//! (`--tools`, `--allowedTools`, `--add-dir`), so that what an agent may use is the flow's, and the installation gives
//! no argument. The files of the worktree and of the temporary directory are written the same way, as an absolute
//! rule and a directory: a rule relative to the current directory would follow the agent's `cd` and refuse a write
//! outside the directory it moved to, which in a print-mode run no one can allow.
//!
//! On every line two kinds of command are refused: what every agent is refused whatever its project, pushing and the
//! agents' own program the installation names (`agents/refused.toml` beside the flows), and the heavy commands of the
//! project's declaration (`[heavy]`), its compiling, type checking, building, test suites and benchmarks. The project's
//! tests are had through the tool `run_tests`, and only the engine pushes; run by an agent, they would fight the owner
//! for the processor, and push what no check passed; and the program run by an agent would spend where nothing counts
//! it. A line that only reads, as its line in the flow says, is refused the tools that edit files and the git commands
//! that change the repository; those that only read, such as `show`, `diff` and `log`, it may run. Those two lists are
//! the engine's. A line that changes only the project's tests is refused nothing more than any line that writes, the
//! look at its run's change as each round ends holding it to them (`tests_only`); it is told the paths of the tests.
//!
//! The command line refuses what any rule refuses even where another allows it, so a refused command stays refused
//! though `shell` lets the agents run shell commands.

use std::path::Path;

use flow::agents::{Ability, Agents, Declaration, PROGRAM_SLOT, YourLine};

/// The commands a round of the project whose declaration this is is refused, each a command with the start of its
/// arguments: what every agent is refused whatever its project, the program the installation names, `program`, in
/// the place of the slot that stands for it, then the project's heavy commands.
pub fn refused_commands(beside: &Agents, declaration: &Declaration, program: &str) -> Vec<String> {
    (beside.refused.commands.iter())
        .map(|command| command.replace(PROGRAM_SLOT, program))
        .chain(declaration.heavy.commands.iter().cloned())
        .collect()
}

/// What is wrong with what every agent is refused whatever its project, said with where it is written: an entry that
/// names nothing, or holds a `*` or a parenthesis; and the program the installation names, `program`, which is
/// refused as an entry is, and so must be written as one. It is the same for every project.
pub fn faults(beside: &Agents, program: &str) -> Vec<String> {
    let mut faults = shape(&refused_at(beside), &beside.refused.commands);
    if program.trim().is_empty() {
        faults.push("agents.command: names the program that runs the agents' rounds".to_string());
    }
    if program.contains(['(', ')', '*']) {
        faults.push(format!("agents.command: {program:?} holds a `*` or a parenthesis"));
    }
    faults
}

/// Where what every agent is refused is written.
fn refused_at(beside: &Agents) -> String {
    beside.source(&format!("{}/{}", flow::agents::AGENTS_DIR, flow::agents::REFUSED_FILE))
}

/// What is wrong with a project's heavy commands, `[heavy]` of its declaration, each said with where it is written: a
/// command that names nothing, or holds a `*` or a parenthesis.
pub fn heavy_faults(declaration: &Declaration) -> Vec<String> {
    let at = format!("{} [heavy]", flow::agents::DECLARATION);
    shape(&at, &declaration.heavy.commands)
}

/// What is wrong with the shape of these commands, written at `at`: one that names nothing, and one that holds a `*` or
/// a parenthesis, which would stand for any text or end the rule it is written into.
fn shape(at: &str, commands: &[String]) -> Vec<String> {
    let mut faults = Vec::new();
    if commands.iter().any(|c| c.trim().is_empty()) {
        faults.push(format!("{at}: each entry names a command"));
    }
    if let Some(c) = commands.iter().find(|c| c.contains(['(', ')', '*'])) {
        faults.push(format!("{at}: {c:?} holds a `*` or a parenthesis"));
    }
    faults
}

/// The tools that edit files, which a line that only reads is refused.
pub const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// The git commands that change the repository, which a line that only reads is refused.
pub const GIT_CHANGES: [&str; 19] = [
    "add",
    "commit",
    "reset",
    "checkout",
    "switch",
    "restore",
    "stash",
    "merge",
    "rebase",
    "rm",
    "mv",
    "cherry-pick",
    "revert",
    "tag",
    "branch",
    "worktree",
    "apply",
    "am",
    "clean",
];

/// What the agents of a line may do and run, as the command line's flags name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permissions {
    /// The line's agents only read.
    pub reads_only: bool,
    /// The tools the round is given, as `--tools` names them: those its words let it use.
    pub tools: Vec<String>,
    /// What a round may use without being asked, as the command line's tool rules name it.
    pub allowed: Vec<String>,
    /// The directories beyond the worktree the round may reach, as `--add-dir` names them.
    pub directories: Vec<String>,
    /// What a round may not use, as the command line's tool rules name it.
    pub refused: Vec<String>,
}

impl Permissions {
    /// The permissions of a line that only reads or that writes: what the words of `allowed.toml`, `may`, let it do,
    /// with `worktree` the work's worktree, `temp_dir` the temporary directory the engine hands the program, and the
    /// commands a round of its project is refused, `commands` ([`refused_commands`]), each refused whole, whatever a
    /// word allows.
    pub fn of(reads_only: bool, may: &[Ability], worktree: &Path, temp_dir: &Path, commands: &[String]) -> Permissions {
        let (mut tools, mut allowed, mut directories) = (Vec::new(), Vec::new(), Vec::new());
        let add = |list: &mut Vec<String>, items: &[&str]| {
            for item in items {
                if !list.iter().any(|have| have == item) {
                    list.push(item.to_string());
                }
            }
        };
        // A directory without its trailing separators, as the rule and the flag write it: `Edit(//var/T/**)` is the
        // absolute path `/var/T`, and a trailing `/` of `$TMPDIR` would write it twice.
        let bare = |dir: &Path| dir.to_string_lossy().trim_end_matches('/').to_string();
        let (worktree, temp) = (bare(worktree), bare(temp_dir));
        let worktree_rule = format!("Edit(/{worktree}/**)");
        let temp_rule = format!("Edit(/{temp}/**)");
        for ability in may {
            match ability {
                Ability::Shell => {
                    add(&mut tools, &["Bash"]);
                    add(&mut allowed, &["Bash"]);
                }
                Ability::Files => {
                    add(&mut tools, &["Read", "Grep", "Glob", "Edit", "Write"]);
                    add(&mut allowed, &["Grep", "Glob", worktree_rule.as_str()]);
                    add(&mut directories, &[worktree.as_str()]);
                }
                Ability::TempFiles => {
                    add(&mut tools, &["Read", "Edit", "Write"]);
                    add(&mut allowed, &[temp_rule.as_str()]);
                    add(&mut directories, &[temp.as_str()]);
                }
                Ability::Web => {
                    add(&mut tools, &["WebSearch", "WebFetch"]);
                    add(&mut allowed, &["WebSearch", "WebFetch"]);
                }
                Ability::Subagents => add(&mut tools, &["Agent"]),
            }
        }
        let mut refused: Vec<String> = commands.iter().map(|c| format!("Bash({c} *)")).collect();
        if reads_only {
            refused.extend(EDIT_TOOLS.iter().map(|t| t.to_string()));
            refused.extend(GIT_CHANGES.iter().map(|g| format!("Bash(git {g} *)")));
        }
        Permissions {
            reads_only,
            tools,
            allowed,
            directories,
            refused,
        }
    }

    /// The arguments that give a round these permissions.
    pub fn arguments(&self) -> Vec<String> {
        let mut out = vec!["--tools".to_string(), self.tools.join(",")];
        if !self.allowed.is_empty() {
            out.push("--allowedTools".to_string());
            out.extend(self.allowed.iter().cloned());
        }
        for directory in &self.directories {
            out.extend(["--add-dir".to_string(), directory.clone()]);
        }
        if !self.refused.is_empty() {
            out.push("--disallowedTools".to_string());
            out.extend(self.refused.iter().cloned());
        }
        out
    }

    /// What the agent is told its line may do in the worktree, in one line of the system prompt, which no rule says by
    /// itself, in the words beside the flows (`agents/your_line.toml`): that it changes no file there, or that it
    /// commits there and, of a line that changes only the project's tests, the paths of those, `tests`; and the paths
    /// the project protects, these, to a line that only reads as well, which judges a change of them.
    pub fn told(&self, words: &YourLine, protected: &[String], tests: Option<&[String]>) -> String {
        if self.reads_only {
            let mut out = words.reads_only.clone();
            if !protected.is_empty() {
                out.push_str(&words.protected_read.replace("{paths}", &listed(protected, &words.and)));
            }
            return out;
        }
        let mut out = words.writes.clone();
        if let Some(tests) = tests {
            out.push_str(&words.tests_only.replace("{paths}", &listed(tests, &words.or)));
        }
        if !protected.is_empty() {
            out.push_str(&words.protected.replace("{paths}", &listed(protected, &words.and)));
        }
        out
    }
}

/// The names, each between backticks, joined by commas, and the last by `last`.
fn listed(names: &[String], last: &str) -> String {
    let named: Vec<String> = names.iter().map(|c| format!("`{c}`")).collect();
    match named.split_last() {
        Some((only, [])) => only.clone(),
        Some((only, rest)) => format!("{}{last}{only}", rest.join(", ")),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;
