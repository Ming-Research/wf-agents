//! What the agents of a line may run: the rules the engine gives every round's command line after the installation's
//! own arguments (`agents.args`), and what the agent is told in its system prompt that no rule says by itself, in the
//! words beside the flows.
//!
//! On every line the heavy commands are refused: compiling, type checking, building, test suites, benchmarks, and
//! pushing. The project's check is had through the tool `check`, and only the engine pushes; run by an agent, they
//! would fight the owner for the processor, and push what no check passed. On a line that writes, the commands that
//! update the project's lock files and build, run and install nothing are allowed, though a heavy command covers them,
//! rather than leave the agent to write a lock file by hand and the check to find its mistakes; which they are is the
//! project's toolchain's, and only the installation's settings name them (`agents.permissions.locks`, with the uses of
//! them refused all the same). A line that only reads, as its line in the flow says, is refused the tools that edit
//! files and the git commands that change the repository; those that only read, such as `show`, `diff` and `log`, it
//! may run. The other lists are the engine's, and an installation's settings may add to them (`agents.permissions`)
//! for its project's toolchain.
//!
//! The command line allows what a rule of the installation's arguments or of these allows, unless a rule of either
//! refuses it, and refuses what any rule refuses even where another allows it. A flag given twice adds its values to
//! those it was given before, as `claude`'s parser of arguments does with every flag that takes a list (checked with
//! claude 2.1.316).

use flow::agents::YourLine;

use crate::settings::PermissionsAdded;

/// The heavy commands, each a command with the start of its arguments: no agent runs them.
pub const HEAVY: [&str; 29] = [
    "cargo",
    "rustc",
    "rustup",
    "make",
    "cmake",
    "ninja",
    "bazel",
    "tsc",
    "npx",
    "pnpm",
    "yarn",
    "bun",
    "deno",
    "npm",
    "node --test",
    "vitest",
    "jest",
    "go build",
    "go test",
    "go run",
    "pytest",
    "python -m pytest",
    "mvn",
    "gradle",
    "swift build",
    "swift test",
    "xcodebuild",
    "hyperfine",
    "git push",
];

/// The heavy commands that no lock command may fall under, as an installation may add one under another heavy
/// command: pushing is the engine's alone.
pub const NEVER_A_LOCK: [&str; 1] = ["git push"];

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

/// What the agents of a line may run, beside what the installation's arguments say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permissions {
    /// The line's agents only read.
    pub reads_only: bool,
    /// The commands that update a lock file its agents may run: none on a line that only reads.
    pub locks: Vec<String>,
    /// What a round may use beside what the installation's arguments allow, as the command line's tool rules name it.
    pub allowed: Vec<String>,
    /// What a round may not use, as the command line's tool rules name it.
    pub refused: Vec<String>,
}

impl Permissions {
    /// The permissions of a line that only reads or that writes, for the installation's arguments `args` and what its
    /// settings add to the engine's lists, `added`.
    ///
    /// A heavy command that a lock command falls under, such as `cargo` for `cargo update`, is not refused whole on a
    /// line that writes, since the command line would then refuse the lock command too. Its other uses are then
    /// refused only as every command is that no rule allows, so each rule of the installation's arguments that allows
    /// one, such as `Bash(cargo fmt *)` or `Bash(cargo f*)`, is refused again. Where those arguments allow all of the
    /// heavy command, as `Bash(carg*)` does, or every command, it is refused whole, and the lock commands under it are
    /// not allowed. The uses of the lock commands that would build, run or install after all are refused wherever a
    /// lock command is allowed.
    pub fn of(reads_only: bool, args: &[String], added: &PermissionsAdded) -> Permissions {
        let heavy: Vec<&str> = HEAVY
            .iter()
            .copied()
            .chain(added.heavy.iter().map(String::as_str))
            .collect();
        let allows = allows(args);
        let rules: Vec<Allow> = allows.iter().filter_map(Allow::of).collect();
        let locks: Vec<String> = if reads_only {
            Vec::new()
        } else {
            (added.locks.iter().map(String::as_str))
                .filter(|l| {
                    !heavy
                        .iter()
                        .any(|h| falls_under(l, h) && rules.iter().any(|r| r.covers(h)))
                })
                .map(str::to_string)
                .collect()
        };
        let allowed = locks.iter().map(|c| format!("Bash({c} *)")).collect();
        let mut refused: Vec<String> = Vec::new();
        for h in &heavy {
            if !locks.iter().any(|l| falls_under(l, h)) {
                refused.push(format!("Bash({h} *)"));
                continue;
            }
            for r in &rules {
                if r.within(h) && !locks.iter().any(|l| r.within(l)) && !refused.contains(r.rule) {
                    refused.push(r.rule.to_string());
                }
            }
        }
        if !locks.is_empty() {
            refused.extend(added.locks_except.iter().map(|c| format!("Bash({c})")));
        }
        if reads_only {
            refused.extend(EDIT_TOOLS.iter().map(|t| t.to_string()));
            refused.extend(GIT_CHANGES.iter().map(|g| format!("Bash(git {g} *)")));
        }
        Permissions {
            reads_only,
            locks,
            allowed,
            refused,
        }
    }

    /// The arguments that give a round these permissions, after the installation's own.
    pub fn arguments(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.allowed.is_empty() {
            out.push("--allowedTools".to_string());
            out.extend(self.allowed.iter().cloned());
        }
        if !self.refused.is_empty() {
            out.push("--disallowedTools".to_string());
            out.extend(self.refused.iter().cloned());
        }
        out
    }

    /// What the agent is told its line may do in the worktree, in one line of the system prompt, which no rule says by
    /// itself, in the words beside the flows (`agents/your_line.toml`): that it changes no file there, or that it
    /// commits there and the commands it may update a lock file with.
    pub fn told(&self, words: &YourLine) -> String {
        if self.reads_only {
            return words.reads_only.clone();
        }
        let mut out = words.writes.clone();
        if !self.locks.is_empty() {
            let named: Vec<String> = self.locks.iter().map(|c| format!("`{c}`")).collect();
            let named = match named.split_last() {
                Some((last, [])) => last.clone(),
                Some((last, rest)) => format!("{}{}{last}", rest.join(", "), words.or),
                None => String::new(),
            };
            out.push_str(&words.locks.replace("{commands}", &named));
        }
        out
    }
}

/// The rules of the command line's tools that the installation's arguments allow, as written there: each value of
/// `--allowedTools` split into its rules, as `claude` splits it, at commas and spaces outside parentheses; and a bare
/// `Bash` when the arguments skip the command line's permission checks or leave them to its own judgment (the
/// permission modes `bypassPermissions` and `auto`), which may then let every command run.
fn allows(args: &[String]) -> Vec<String> {
    const OPEN_MODES: [&str; 2] = ["bypassPermissions", "auto"];
    let mut out = Vec::new();
    let mut taking = false;
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = (a.strip_prefix("--allowedTools=")).or_else(|| a.strip_prefix("--allowed-tools=")) {
            out.extend(rules_in(v));
            taking = false;
        } else if a == "--allowedTools" || a == "--allowed-tools" {
            taking = true;
        } else if a.starts_with('-') {
            taking = false;
            let mode = match a.strip_prefix("--permission-mode=") {
                Some(m) => Some(m),
                None if a == "--permission-mode" => args.get(i + 1).map(String::as_str),
                None => None,
            };
            let open = a == "--dangerously-skip-permissions" || mode.is_some_and(|m| OPEN_MODES.contains(&m));
            if open {
                out.push("Bash".to_string());
            }
        } else if taking {
            out.extend(rules_in(a));
        }
    }
    out
}

/// Whether `command`, a command with the start of its arguments, falls under `heavy`, another: it is that one, or that
/// one with more arguments after it.
pub(crate) fn falls_under(command: &str, heavy: &str) -> bool {
    (command.strip_prefix(heavy)).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// A rule of the command line's tools that lets commands run in its shell, with what it matches: every command that
/// is `command` or `command` with more arguments after it where it is `bounded`; every command whose text begins with
/// `command` where it is not, as `Bash(carg*)` matches `cargo` and `cargox`. An empty command, which a bare `Bash`
/// or a rule that starts with `*` has, may match every command.
struct Allow<'a> {
    rule: &'a String,
    command: &'a str,
    bounded: bool,
}

impl<'a> Allow<'a> {
    /// What the rule matches: `Bash(cargo fmt *)`, `Bash(cargo fmt:*)` and the exact `Bash(cargo fmt)` name `cargo fmt`
    /// at a word's end; a `*` anywhere else makes what comes before it a beginning of any text, as in `Bash(carg*)` or
    /// `Bash(git * push)`. None for a rule of another tool.
    fn of(rule: &'a String) -> Option<Allow<'a>> {
        if rule == "Bash" {
            return Some(Allow {
                rule,
                command: "",
                bounded: false,
            });
        }
        let body = rule.strip_prefix("Bash(")?.strip_suffix(')')?;
        let (command, bounded) = match body.split_once('*') {
            None => (body.trim(), true),
            Some((head, _)) => match head.strip_suffix(':') {
                Some(head) => (head.trim(), true),
                None if head.ends_with(char::is_whitespace) => (head.trim(), true),
                None => (head, false),
            },
        };
        Some(Allow {
            rule,
            command,
            bounded: bounded && !command.is_empty(),
        })
    }

    /// Whether the rule matches every use of `heavy`, a command with the start of its arguments.
    fn covers(&self, heavy: &str) -> bool {
        match self.bounded {
            true => falls_under(heavy, self.command),
            false => heavy.starts_with(self.command),
        }
    }

    /// Whether everything the rule matches falls under `command` (one that matches every use of it among them).
    fn within(&self, command: &str) -> bool {
        match self.bounded {
            true => falls_under(self.command, command),
            false => self.command.starts_with(&format!("{command} ")),
        }
    }
}

/// The rules a value of a list of tool rules holds, split at commas and spaces outside parentheses.
fn rules_in(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    for (i, c) in value.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 && (c == ',' || c.is_whitespace()) => {
                out.push(&value[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&value[start..]);
    out.into_iter().filter(|r| !r.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod tests;
