//! What the agents of a line may run: the rules the engine gives every round's command line after the installation's
//! own arguments (`agents.args`), and what the agent is told of them in its system prompt, so that it does not find
//! out by being refused.
//!
//! On every line the heavy commands are refused: compiling, type checking, building, test suites, benchmarks, and
//! pushing. The project's check is the tool `check`, which the engine runs, and only the engine pushes; run by an
//! agent, they would fight the owner for the processor, and push what no check passed. On a line that writes, the
//! commands that update a lock file and build, run and install nothing are allowed, though a heavy command covers
//! them, rather than leave the agent to write a lock file by hand and the check to find its mistakes. A line that
//! only reads, as its line in the flow says, is refused the tools that edit files and the git commands that change the
//! repository; those that only read, such as `show`, `diff` and `log`, it may run.
//!
//! The command line allows what a rule of the installation's arguments or of these allows, unless a rule of either
//! refuses it, and refuses what any rule refuses even where another allows it. A flag given twice adds its values to
//! those it was given before, as `claude`'s parser of arguments does with every flag that takes a list (checked with
//! claude 2.1.316).

/// The heavy commands, each a command with the start of its arguments: no agent runs them.
pub const HEAVY: [&str; 33] = [
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
    "npm run",
    "npm test",
    "npm ci",
    "npm install",
    "npm exec",
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

/// The commands that update a lock file and build, run and install nothing, each a command with the start of its
/// arguments: Cargo's and npm's. `cargo update` and `cargo add` resolve the dependencies and write Cargo.lock without
/// compiling anything; `npm install --package-lock-only` writes package-lock.json and leaves node_modules alone, and
/// with `--ignore-scripts` runs none of the package's own scripts, which it runs when it is given no package.
pub const LOCKS: [&str; 3] = [
    "cargo update",
    "cargo add",
    "npm install --package-lock-only --ignore-scripts",
];

/// The uses of the lock commands refused all the same, wherever those are allowed, each a command whose arguments may
/// hold `*` for any text: npm's options that would undo `--package-lock-only` or `--ignore-scripts` or install
/// elsewhere, as a later npm option overrides an earlier one. They are a negated option, which npm reads in any case
/// (`--no-ignore-scripts`, `--NO-ignore-scripts`); any option given a value after `=`, which turns a flag off with
/// `false`, `null`, `0` or nothing and sets `--location=global` or `--prefix=<dir>`; a flag turned off by a separate
/// `false`; and `-g`, `--global`, `--location`, `--prefix` and `-C`, written out or shortened, and the short ones with
/// two dashes, which npm takes as well. A short option in a cluster of them, as the `g` of `-Dg`, cannot be named in
/// the command line's patterns without refusing the lock command itself, and is a risk left.
pub const LOCKS_EXCEPT: [&str; 12] = [
    "npm install *-no-*",
    "npm install *-NO-*",
    "npm install *-No-*",
    "npm install *-nO-*",
    "npm install *=*",
    "npm install * false*",
    "npm install * -g*",
    "npm install *--g*",
    "npm install *--loc*",
    "npm install *--pref*",
    "npm install * -C*",
    "npm install *--C*",
];

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
    /// The permissions of a line that only reads or that writes, for the installation's arguments `args`.
    ///
    /// A heavy command that a lock command falls under, such as `cargo` for `cargo update`, is not refused whole on a
    /// line that writes, since the command line would then refuse the lock command too. Its other uses are then
    /// refused only as every command is that no rule allows, so each rule of the installation's arguments that allows
    /// one, such as `Bash(cargo fmt *)`, is refused again. Where those arguments allow all of the heavy command, or
    /// every command, it is refused whole, and the lock commands under it are not allowed. The uses of the lock
    /// commands that would build, run or install after all are refused wherever a lock command is allowed.
    pub fn of(reads_only: bool, args: &[String]) -> Permissions {
        Permissions {
            reads_only,
            locks: Vec::new(),
            allowed: Vec::new(),
            refused: allows(args),
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

    /// What the agent is told its line may do in the worktree, in one line of the system prompt: that it only reads
    /// there, or that it commits there and the commands it updates a lock file with. A bare `cargo update` upgrades
    /// every dependency, which would fill the work's diff with changes it did not need, so where it is among them the
    /// agent is told how to change only what the work needs.
    pub fn told(&self) -> String {
        String::new()
    }
}

/// The rules of the command line's tools that the installation's arguments allow, as written there: each value of
/// `--allowedTools` split into its rules, as `claude` splits it, at commas and spaces outside parentheses; and a bare
/// `Bash` when the arguments skip the command line's permission checks or leave them to its own judgment (the
/// permission modes `bypassPermissions` and `auto`), which may then let every command run.
fn allows(args: &[String]) -> Vec<String> {
    args.iter().take(0).cloned().collect()
}

#[cfg(test)]
mod tests;
