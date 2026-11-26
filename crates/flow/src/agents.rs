//! What the engine says to every agent, whatever the flow, as it is written beside the flows, in `agents/`: what a
//! session begins with, what every agent is refused whatever its project, the rules for the agents, the engine's own
//! words, the return and its fields, the tools, the message a cell is sent as it is entered (design document
//! `data-and-context`, 14), and what the engine writes as it works: into the blocks its acts write, in the commits it
//! makes, on the pull requests of the works, and as the result of a call it ends itself. Every word an agent is sent,
//! the host is written, or the owner's cards say that is not a flow's is in these files; the code holds none of them,
//! only how they are put together. A field a file does not know is refused, a text that is empty is refused, and what
//! cannot be read is said with the path of its file. What is a project's (its files, settings, tests, workflows, heavy
//! commands and how its host checks and merges) is not beside the flows but in the project's own declaration
//! ([`Declaration`]), at the root of its repository.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{InputKind, Listed};

/// The directory beside the flows that holds what the engine says to every agent.
pub const AGENTS_DIR: &str = "agents";

/// A project's declaration, by its path from the root of the project's repository: each project an installation names
/// declares its files, its settings, its tests, its workflows, its heavy commands and how its host checks and merges
/// there, and the works of its items are run with it. No copy of it is beside the flows.
pub const DECLARATION: &str = "wf-agents.toml";

/// The file, from the directory that holds the flows, that names the sets of rules for the agents, each by its name
/// with its file beside it. A line names those it takes.
pub const RULES_FILE: &str = "agents/rules/rules.toml";

/// The directory, in `agents/`, of what the engine's acts write into the blocks their cells write: a file to an act,
/// named by it.
pub const ACTS_DIR: &str = "acts";

/// The file, in `agents/`, of who the engine's commits are by and the words of each.
pub const COMMITS_FILE: &str = "commits.toml";

/// The file, in `agents/`, of what the texts beside the flows take from the installation: the settings of it they may
/// name, and what a tool's result says where the work's item belongs to no project the installation names.
pub const INSTALLATION_FILE: &str = "installation.toml";

/// The file, in `agents/`, of what every agent is refused whatever its project.
pub const REFUSED_FILE: &str = "refused.toml";

/// The file, in `agents/`, of the design tree's format and of the words of each fault in the forms of its files: the
/// tree's rules, the same for every project.
pub const TREE_FILE: &str = "tree.toml";

/// The slot in an entry of [`REFUSED_FILE`] that stands for the agents' program the installation names
/// (`agents.command`): the only slot the file may hold, filled where the refusals are made.
pub const PROGRAM_SLOT: &str = "{agents.command}";

/// The file, in `agents/`, of what every agent may do, in words that name no program.
pub const ALLOWED_FILE: &str = "allowed.toml";

/// The file, in `agents/`, of what the place `{your_line}` in the engine's words says: what the line may do in the
/// worktree.
pub const YOUR_LINE_FILE: &str = "your_line.toml";

/// The texts the engine fills of a tool, by the tool: its file holds each of these and no other, since the engine says
/// each of them in some answer and a misspelt name would leave an answer empty. A tool not named here has none.
pub const TOOL_TEXTS: &[(&str, &[&str])] = &[(
    "query",
    &[
        "unreadable",
        "error",
        "timeout",
        "unavailable",
        "row_too_long",
        "store_table",
        "store_column",
        "store_column_commented",
    ],
)];

/// The file, in `agents/`, of the questions the engine itself asks the owner.
pub const ASKS_FILE: &str = "asks.toml";

/// The file, in `agents/`, of what the engine writes on a work's pull request where the project's host is GitHub.
pub const PULL_REQUEST_FILE: &str = "pull_request.toml";

/// The file, in `agents/`, of the result a call has where the engine ends it itself.
pub const CALLS_FILE: &str = "calls.toml";

/// The places in the engine's words that the engine fills with a number as it is written: an item's as `#n`, a work's
/// as `Wn`. A text writes no `W` or `#` before one of them.
pub const NUMBER_PLACES: &[&str] = &["work", "item", "items", "last"];

/// The file, in `agents/`, of what the engine says to the owner of why a work waits on them, and why it paused a work
/// as a round of it failed. It is read with each flow, which carries it ([`crate::Compiled::waits`]), so that the
/// engine's deciding and showing read the words of the flow a work runs on.
pub const WAITS_FILE: &str = "waits.toml";

/// `agents/waits.toml`: why a work waits on the owner, and why the engine paused a work as a round of it failed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waits {
    pub paused: PausedWords,
    pub owner: OwnerWords,
    pub stalled: StalledWords,
}

/// Why a stalled work rests, made of these in one sentence: what it waits for and which cells could make it hold, then
/// why each of them does not run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StalledWords {
    /// The cell it stands at waits for `{keys}`, which `{writers}` can make hold.
    pub waits: String,
    /// It stands at its last cell, which nothing new runs, and `{writers}` can write what runs it.
    pub last: String,
    /// The cells that write what it waits for, `{cells}`.
    pub only: String,
    /// No cell writes what it waits for.
    pub none: String,
    /// `{cell}` does not run, as `{why}` says.
    pub unrun: String,
    /// It is skipped, its `when` keys `{keys}` not holding.
    pub skipped: String,
    /// Nothing new runs it.
    pub nothing_new: String,
    /// The cell the work stands at holds it.
    pub held: String,
    /// A key or a cell, `{name}`.
    pub name: String,
    pub and: String,
    pub or: String,
    pub then: String,
}

/// Why the engine paused a work, each with `{why}`, what carried the round out, or the act that lets go of what the
/// work held, said.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PausedWords {
    /// The prompt is too long for the model.
    pub too_long: String,
    /// The round failed in passing `{failed}` times in a row.
    pub passing: String,
    /// The round failed `{failed}` times in a row for a cause outside every work.
    pub outside: String,
    /// The service's safety check flagged the round again, in the new session of its line.
    pub flagged_again: String,
    /// Letting go of what an ended work held failed.
    pub let_go: String,
    /// The round failed, for a cause no other of these says: `{why}` it.
    pub failed: String,
    /// The service has no model the level a round runs on names: `{model}`, `{level}`, `{work}` the work whose round
    /// found it, `{why}` what the service said.
    pub no_model: String,
}

/// Why a work waits on the owner, a sentence for each thing it waits for; a question is said by what it asks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerWords {
    pub limit: String,
    pub calls: String,
    pub returns: String,
    pub made: String,
    pub ruling: String,
    pub paused: String,
    pub no_reason: String,
    pub held: String,
    pub agents: String,
    /// What the owner does where the agents' command line cannot run, outside the page.
    pub mend: String,
}

/// The field of a return that holds the blocks a cell writes, each under its name, in its own shape. The form of a
/// line lists the blocks of the line's cells there; the cell is told its own with its task, and the engine sends back
/// a block that is not among them.
pub const OUTPUTS: &str = "outputs";

/// All of it, read from the directory that holds the flows.
#[derive(Clone, Debug)]
pub struct Agents {
    /// The directory read, which holds the flows.
    pub root: PathBuf,
    pub session: Session,
    /// The text of each file of the engine's own words that the session names, by its name in `agents/`.
    pub words: BTreeMap<String, String>,
    pub your_line: YourLine,
    /// What the texts take from the installation: `agents/installation.toml`.
    pub installation: Installation,
    /// What every agent is refused whatever its project: [`REFUSED_FILE`].
    pub refused: Commands,
    /// What every agent may do, in words that name no program: [`ALLOWED_FILE`].
    pub allowed: Allowed,
    /// The sets of rules for the agents, by name, as `agents/rules/rules.toml` names them, each with its text.
    pub rules: BTreeMap<String, RuleSet>,
    pub returns: Return,
    /// What a round is told of a return sent back: `agents/return/sent_back.toml`.
    pub sent_back: SentBack,
    /// Each field of a return that is the engine's, by name: `agents/return/fields/<name>.toml`.
    pub fields: BTreeMap<String, Field>,
    /// Each tool of the engine's, by name: `agents/tools/<name>.toml`.
    pub tools: BTreeMap<String, Tool>,
    pub message: Message,
    pub given: Given,
    /// What a round is told of the results of the calls it is given: `agents/message/results.toml`.
    pub results: Results,
    pub acts: Acts,
    pub commits: Commits,
    /// What the engine itself asks the owner: `agents/asks.toml`.
    pub asks: Asks,
    /// What the engine says as it guards what rounds cost and keeps their sessions' caches: `agents/cost.toml`.
    pub cost: Cost,
    /// What the engine writes on a work's pull request: [`PULL_REQUEST_FILE`].
    pub pull_request: PullRequest,
    /// The result a call has where the engine ends it itself: [`CALLS_FILE`].
    pub calls: Calls,
}

/// [`PULL_REQUEST_FILE`]: what the engine writes on a work's pull request where the project's host is GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullRequest {
    /// What opens the body where the work wrote of itself: `{about}` that, in a block of plain text.
    pub about: String,
    /// The rest of the body: `{work}` the work's number.
    pub body: String,
    /// What follows, on a line of its own, what the work wrote of itself where it is cut.
    pub cut: String,
    /// The title of the commit the merge makes: `{name}` the name of the work's merge, `{number}` the pull request's
    /// number on the host.
    pub merged: String,
    /// The comment it is closed with, where its work ended without merging.
    pub closed: String,
    /// The comment the pull request an item continued is closed with, once the work's own is open: `{url}` the address
    /// of the work's own.
    pub superseded: String,
    pub withheld: Withheld,
}

/// What stands, in what is published, in place of each path of this machine.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Withheld {
    pub data: String,
    pub repository: String,
    pub home: String,
}

/// [`CALLS_FILE`]: the result a call has where the engine ends it itself, kept as its result in the work's records.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calls {
    /// Of a call withdrawn as its run was abandoned, before it had its result.
    pub withdrawn: String,
    /// Of the engine's wait before a round that failed in passing is tried again, once its time has come.
    pub waited: String,
    /// What a call of a tool the engine does not have is answered: `{name}`, `{tools}`.
    pub no_such_tool: String,
}

/// `agents/cost.toml`: what the engine says as it runs the agents' program, guards what its rounds cost and keeps their
/// sessions' prompt caches.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub program: ProgramWords,
    pub check: CheckWords,
    pub ping: PingWords,
    pub fork: ForkWords,
    pub guard: GuardWords,
    pub work: WorkWords,
}

/// Why a run of the agents' program failed where it ended with no result to read: a round's, a ping's or the check of
/// the outside's.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramWords {
    /// It exited with a code.
    pub exited: String,
    /// A signal ended it.
    pub signal: String,
    /// It ran as the process of the engine that ran it ended, and what it printed until then holds no result.
    pub cut_off: String,
    /// Its stream holds no result.
    pub no_stream_result: String,
    /// Its result cannot be read: `{why}` what the reading said.
    pub unreadable_result: String,
    /// Its result holds no structured return.
    pub no_return: String,
    /// Its result is an error of a kind it names: `{subtype}` that kind, `{said}` what it said.
    pub error_kind: String,
    /// Its result is an error that names no kind: `{said}` what it said.
    pub error: String,
    /// Why there is no return where its last attempt at one is no JSON: `{reason}` why there is none, as a sentence,
    /// `{error}` what is wrong with the attempt, `{refused}` what `refused` makes of what the program said of it.
    pub last_attempt: String,
    /// Where the program said why it refused the attempt: `{said}` what it said.
    pub refused: String,
    /// The result is longer than is read of one: `{long}` its length in bytes, `{most}` what is read.
    pub too_long: String,
    /// The last lines of the stream are longer than is read: `{most}` what is read.
    pub lines_too_long: String,
    /// What the stream holds is no result: `{why}` what the reading said.
    pub not_a_result: String,
    /// The program said it failed and gave no reason.
    pub no_reason: String,
}

/// What the engine's check of the outside of the agents' rounds found down, where it says it itself.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckWords {
    /// No account is in use, which the check is taken under: nothing is run.
    pub no_account: String,
    /// The network does not reach the agents' service.
    pub offline: String,
    /// What the check asks the agents' service, which it answers: the whole request.
    pub request: String,
}

/// What a keep-warm ping sends, and why one failed.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingWords {
    /// The whole message it sends.
    pub message: String,
    pub under_way: String,
    pub no_round: String,
    pub no_system: String,
    pub stopped: String,
}

/// What a fork of a round's session is sent before its question.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkWords {
    /// The whole message it is sent first.
    pub first: String,
}

/// Where a line the run guard judges by comes from, by the scope of its history.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeWords {
    pub cell: String,
    pub flow: String,
    pub works: String,
    pub line: String,
}

/// Why the run guard stopped a round.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardWords {
    /// That it stopped the round, a sentence before what it measured.
    pub stopped: String,
    /// That it stopped the run's round again, in the new session its line was given, in place of `stopped`.
    pub again: String,
    /// Between that and what it measured.
    pub then: String,
    pub growth: String,
    pub jump: String,
    pub from: String,
    pub scope: ScopeWords,
    pub tool: String,
    pub between: String,
    pub no_tools: String,
}

/// Why the engine paused a work whose spend or time is past the line of the flow's other works.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkWords {
    pub spent: String,
    pub time: String,
    /// What it has spent is over the budget the settings give.
    pub budget: String,
    pub started: String,
    pub let_on: String,
    pub between: String,
    /// That the round failed too, where the work is paused for its budget as well: `{why}` its cause.
    pub failed: String,
}

impl Agents {
    /// The path of a file of these, from the directory that holds the flows, with that directory's name before it, as
    /// a part names the file it comes from.
    pub fn source(&self, path: &str) -> String {
        let root = self.root.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        format!("{root}/{path}")
    }
}

/// `wf-agents.toml` at the root of a project's repository: what is the project's: where its design tree is, its files
/// the engine gives every agent, its settings the words sent to the agents may name, its tests, its workflows, its heavy commands,
/// which no agent runs, the variables its toolchain reads, which every agent's command line is given, and what its host
/// needs to check and merge: the command that checks it where its repository is the host, and how its pull requests
/// are merged where GitHub is.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The directory of the project's checkout that holds its design tree, from the root of the checkout.
    pub tree: String,
    /// How an entry of the design tree is found and referred to, as a declaration written before the tree's format
    /// became the flow's gave it, which an engine installed before that still reads. This engine does not use it: it is
    /// refused where it differs from the flow's format, so that it cannot be a second one.
    #[serde(default)]
    pub tree_format: Option<TreeFormat>,
    /// The project's files, by the name a part of a session gives them.
    pub files: BTreeMap<String, ProjectFile>,
    /// The project's settings, which the words sent to the agents name as `project.<name>`.
    #[serde(default)]
    pub project: BTreeMap<String, String>,
    /// Where the project's tests are, and what its tests expect.
    pub tests: Option<Tests>,
    /// The project's workflows, which the engine runs tests, checks and measurements through.
    pub ci: Option<Ci>,
    /// How the project is checked where its own repository is the host: needed there, and read nowhere else.
    pub local: Option<Local>,
    /// How its pull requests are merged where GitHub is the host: needed there, and read nowhere else.
    pub pull_request: Option<Merging>,
    /// The paths the project protects; none where it protects none.
    #[serde(default)]
    pub protected: Protected,
    /// The project's heavy commands, which no agent runs. A declaration names them, none where its project has none,
    /// so that one that leaves them out is refused rather than taken to refuse nothing.
    pub heavy: Heavy,
    /// The variables, by name, that every agent's command line of the project's works is given after the engine's own:
    /// what the project's toolchain reads, so that a command an agent may run reaches no further than its worktree. A
    /// declaration names them, none where its project needs none, as it names its heavy commands, so that one that
    /// leaves them out is refused rather than taken to need none.
    pub environment: BTreeMap<String, String>,
}

/// `[local]` of the declaration: how the project is checked where its own repository is the host.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Local {
    /// A shell command whose exit with 0 means the work passes: run on a checkout of the commit to merge by the check
    /// before the merge, and on the worktree by the tool `run_tests`.
    pub check: String,
}

/// The ways a pull request may be merged.
pub const MERGE_METHODS: [&str; 3] = ["squash", "merge", "rebase"];

/// `[pull_request]` of the declaration: how the project's pull requests are merged where GitHub is the host. The words
/// the engine writes on a pull request are [`PullRequest`], beside the flows.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Merging {
    /// How a pull request is merged: one of [`MERGE_METHODS`].
    pub merge_method: String,
    /// Whether a work's branch on GitHub is deleted once it is merged.
    pub delete_branch: bool,
}

/// `[heavy]` of the declaration: the project's heavy commands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heavy {
    /// The heavy commands, each a command with the start of its arguments: no agent runs them.
    pub commands: Vec<String>,
}

impl Declaration {
    /// The declaration, read from the text of its file, read at `at`, against what the session begins with: or why it
    /// cannot be read, or lacks a file the session names, said with where it was read.
    pub fn read(text: &str, at: &str, beside: &Agents) -> Result<Declaration, String> {
        let declaration: Declaration =
            toml::from_str(text).map_err(|e| format!("{at}: {}", e.to_string().trim_end()))?;
        let faults = declaration.faults(beside);
        if faults.is_empty() {
            Ok(declaration)
        } else {
            Err(format!("{at}: {}", faults.join("; ")))
        }
    }

    /// What is wrong with the declaration whatever the project's host is, each in words with its key: a format of the
    /// design tree that is not the flow's, a file the session names that it does not have, a merge method the host
    /// does not take, a check that names no command, and a workflow's time limit that is not a time.
    pub fn faults(&self, beside: &Agents) -> Vec<String> {
        let mut faults = Vec::new();
        if let Some(format) = &self.tree_format
            && *format != beside.acts.tree.format
        {
            let tree = beside.root.join(AGENTS_DIR).join(TREE_FILE);
            faults.push(format!(
                "tree_format: is not the format of the design tree in [format] of {}, which is the same for every project",
                tree.display()
            ));
        }
        for part in &beside.session.parts {
            if let Some(name) = part.file()
                && !self.files.contains_key(name)
            {
                let session = beside.root.join(AGENTS_DIR).join("session.toml");
                faults.push(format!("has no file {name}, which {} names", session.display()));
            }
        }
        if let Some(pull) = &self.pull_request
            && !MERGE_METHODS.contains(&pull.merge_method.as_str())
        {
            faults.push(format!(
                "pull_request.merge_method: one of {}, not {:?}",
                MERGE_METHODS.join(", "),
                pull.merge_method
            ));
        }
        if self.local.as_ref().is_some_and(|local| local.check.trim().is_empty()) {
            faults.push("local.check: names a command".to_string());
        }
        if let Some(ci) = &self.ci {
            let measure = ci
                .measure
                .as_ref()
                .map(|measure| ("ci.measure.too_long", &measure.too_long));
            for (key, text) in [
                ("ci.fast.too_long", &ci.fast.too_long),
                ("ci.full.too_long", &ci.full.too_long),
            ]
            .into_iter()
            .chain(measure)
            {
                if let Err(why) = span(text) {
                    faults.push(format!("{key}: {why}"));
                }
            }
        }
        faults
    }
}

/// A time such as `90m`, `2h`, `30s` or `1d`; or why it is none: one whose seconds do not fit in a `u64` is none, so
/// that a declaration, which agents write, cannot make the engine overflow or wrap to a short time.
pub fn span(text: &str) -> Result<std::time::Duration, String> {
    let text = text.trim();
    let none = || format!("{text:?} is no time such as 90m or 2h");
    let (number, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()));
    let number: u64 = number.parse().map_err(|_| none())?;
    let seconds: u64 = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return Err(none()),
    };
    let seconds = number.checked_mul(seconds).ok_or_else(none)?;
    Ok(std::time::Duration::from_secs(seconds))
}

/// `[format]` of `agents/tree.toml`: how an entry of a design tree is found and referred to, as the tree's rules write
/// them, which are the same for every project. An entry begins at a line in the form of
/// `heading` and runs to the line before the next line in that form, of any kind, or to its file's end, less the blank
/// lines at its end; a text refers to it in the form of `reference`. In both, `{kind}` stands for a kind's name and
/// `{name}` for the entry's, which holds no white space.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeFormat {
    /// Each kind of entry, in the order the tree's rules read them.
    pub kinds: Vec<EntryKind>,
    /// The line an entry begins with.
    pub heading: String,
    /// How a text refers to an entry.
    pub reference: String,
}

/// A kind of entry of the design tree, and the directory under the tree that holds its files.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryKind {
    pub kind: String,
    pub dir: String,
}

/// What stands for a kind's name, and for an entry's, in the forms of a [`TreeFormat`].
const KIND: &str = "{kind}";
const NAME: &str = "{name}";

impl TreeFormat {
    /// What is wrong with the format, each in words: no kind, a kind or a directory that is empty, and a form that
    /// lacks `{kind}` or `{name}`.
    pub fn faults(&self) -> Vec<String> {
        let mut faults = Vec::new();
        if self.kinds.is_empty() {
            faults.push("format.kinds names no kind".to_string());
        }
        for (i, kind) in self.kinds.iter().enumerate() {
            if kind.kind.trim().is_empty() || kind.dir.trim().is_empty() {
                faults.push(format!("format.kinds[{i}] must give a kind and a directory"));
            }
        }
        for (field, form) in [("heading", &self.heading), ("reference", &self.reference)] {
            for place in [KIND, NAME] {
                if !form.contains(place) {
                    faults.push(format!("format.{field} must hold {place}"));
                }
            }
        }
        faults
    }

    /// The kind and the name of the entry this line of the tree begins, where it begins one.
    pub fn heading_of<'l>(&self, line: &'l str) -> Option<(&str, &'l str)> {
        self.matched(&self.heading, line.trim_end())
    }

    /// The kind and the name of the entry this line refers to, where it is written as the format refers to one.
    pub fn referred<'l>(&self, line: &'l str) -> Option<(&str, &'l str)> {
        self.matched(&self.reference, line.trim())
    }

    /// The kind and the name a line holds in this form, the first kind that fits it.
    fn matched<'l>(&self, form: &str, line: &'l str) -> Option<(&str, &'l str)> {
        self.kinds.iter().find_map(|kind| {
            let form = form.replace(KIND, &kind.kind);
            let (before, after) = form.split_once(NAME)?;
            let name = line.strip_prefix(before)?.strip_suffix(after)?;
            (!name.is_empty() && !name.contains(char::is_whitespace)).then_some((kind.kind.as_str(), name))
        })
    }
}

/// The kinds of entry the forms of the tree's files are checked for: the check has the form of each by its name.
pub const FORMED_KINDS: [&str; 2] = ["principle", "decision"];

/// `agents/tree.toml`: the tree's rules as the engine carries them, the same for every project: how an entry is found
/// and referred to, and the words of each fault found in the forms of the tree's files.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tree {
    pub format: TreeFormat,
    pub faults: TreeFaults,
}

impl Tree {
    /// What is wrong with the file, each in words: what is wrong with its format, and a kind the forms are checked for
    /// that it lacks.
    pub fn faults(&self) -> Vec<String> {
        let mut faults = self.format.faults();
        for kind in FORMED_KINDS {
            if !self.format.kinds.iter().any(|k| k.kind == kind) {
                faults.push(format!(
                    "format.kinds lacks the kind {kind}, which the forms of the tree's files are checked for"
                ));
            }
        }
        faults
    }
}

/// The words of each fault the check finds in the forms of the tree's files, and of the line that names it. In a text,
/// `{count}` is the number of characters a part holds and `{most}` the most it may; `{kind}` and `{name}` an entry's
/// kind and name; `{dir}` the directory that holds the files of a kind; `{heading}` the line an entry of that kind
/// begins with, as the format writes it, `<name>` standing for its name; `{what}` the work, item or date a line names;
/// `{data}` a path a line gives as its data; `{file}` and `{line}` where a name is taken; `{is}` the kind a name is of.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeFaults {
    /// One fault as a line of what a round is told: `{path}` the file, `{line}` the number of its line, `{entry}` the
    /// words of `entry` where the fault is in an entry and nothing where it is in the file, `{text}` the fault's words.
    pub line: String,
    /// Where a fault is in an entry: `{kind}` and `{name}`.
    pub entry: String,
    /// The file is not text, so that its forms cannot be read.
    pub not_text: String,
    /// A file does not open with its title, a blank line and one line of what it covers.
    pub opens: String,
    /// The line of what a file covers is more than one line, or no blank line follows it.
    pub covers: String,
    /// Two entries are parted by more or less than one blank line, or a blank line ends the file.
    pub blank_lines: String,
    /// An entry does not begin as its directory's kind does: `{dir}`, `{heading}`.
    pub heading: String,
    /// An entry's name is not lowercase words joined by hyphens.
    pub name_shape: String,
    /// A principle is not its rule on one line and its reason on one line after `Why: `.
    pub principle_form: String,
    pub rule_long: String,
    pub why_empty: String,
    pub why_long: String,
    /// A decision's first line is missing or is a line of an approach.
    pub decision_first: String,
    pub first_long: String,
    /// A decision has no line of an approach under its first line.
    pub no_lines: String,
    /// A line under a decision's first is not in the form of a `rejected` or a `replaced` line.
    pub line_form: String,
    pub line_long: String,
    /// A line of a decision names a work, an item or a date: `{what}`.
    pub names_a_moment: String,
    /// A line gives data that the repository does not hold: `{data}`.
    pub data_missing: String,
    /// An entry's name is the name of another entry, which is at `{file}` and `{line}`.
    pub name_taken: String,
    /// A line names an entry no file holds: `{kind}`, `{name}`.
    pub unknown: String,
    /// A line names an entry as another kind than it is: `{kind}`, `{name}`, `{is}`.
    pub wrong_kind: String,
}

/// The paths of the project's checkout that govern its agents, which the owner changes (principle
/// `protected-files-are-the-owners`): where a work's own change touches one, the check before the merge holds it until
/// the owner lets it merge, asking in the words of [`Asks`].
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    /// Each path from the root of the checkout: a file, or a directory, ending in `/`, and everything in it.
    pub paths: Vec<String>,
}

/// Commands, each a command with the start of its arguments: what every agent is refused whatever its project
/// ([`REFUSED_FILE`]). An entry may be [`PROGRAM_SLOT`], the agents' program the installation names.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commands {
    pub commands: Vec<String>,
}

/// [`ALLOWED_FILE`]: what every agent may do, whatever its project, in words that name no program, tool or path; the
/// engine's adapter for the agents' program turns each word into that program's own arguments. A word the file does
/// not know is refused when it is read.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allowed {
    /// The words, each once.
    pub may: Vec<Ability>,
}

/// One thing an agent may do, as a word of [`ALLOWED_FILE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ability {
    /// Run shell commands in the worktree, beyond what is refused.
    Shell,
    /// Find, read and edit files in the worktree.
    Files,
    /// Read and edit files in the temporary directory the engine hands the program.
    TempFiles,
    /// Search the web and fetch pages.
    Web,
    /// Start subagents.
    Subagents,
}

impl Ability {
    /// The word of [`ALLOWED_FILE`] that names it.
    pub fn word(self) -> &'static str {
        match self {
            Ability::Shell => "shell",
            Ability::Files => "files",
            Ability::TempFiles => "temp_files",
            Ability::Web => "web",
            Ability::Subagents => "subagents",
        }
    }
}

/// `[tests]` of the declaration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    /// Where the project's tests are, as git pathspecs: the files a line that changes only the tests may change.
    pub paths: Vec<String>,
    /// The expected-value data of its tests, as git pathspecs.
    pub expected: Vec<String>,
}

/// `[ci]` of the declaration: the project's two workflows, each with a role the engine relies on, and its measure
/// workflow where it has one. All report each test, or each timing, as JUnit XML, named by the one name an agent asks for
/// it by. A declaration that has `[ci]` has both the fast and the full: the fast one is all that `run_tests` runs for the
/// tests and checks it is asked for, and the full ones are all that the check before a merge runs; the measure workflow
/// is all that `run_tests` runs for the measurements it is asked for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ci {
    pub fast: CiFast,
    pub full: CiFull,
    /// The workflow that times what a request of `run_tests` names; none where the project has no measurements.
    #[serde(default)]
    pub measure: Option<CiMeasure>,
}

/// The measure workflow: dispatched by hand on a commit and the commit the work's change began from, with the names of
/// the measurements to time, and reports every timing as JUnit XML: a testcase named as the measurement is asked for,
/// in a testsuite named for what was timed, with its time in seconds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiMeasure {
    /// The workflow, by its file's name, always run by its file on the main branch.
    pub workflow: String,
    /// Where it runs, as for the fast workflow: the machine the project measures on.
    pub runner: String,
    /// How long a run may take, such as `30m`, the wait for the machine included: one not complete by then is
    /// cancelled and said so.
    pub too_long: String,
    /// The input that takes the commit to time.
    #[serde(default = "commit_input")]
    pub commit_input: String,
    /// The input that takes the commit the work's change began from, to time it against.
    #[serde(default = "base_input")]
    pub base_input: String,
    /// The input that takes a value only one request knows, which the run's name must hold.
    #[serde(default = "call_input")]
    pub call_input: String,
    /// The input that takes the names of the measurements, each followed by a line break.
    #[serde(default = "measure_input")]
    pub measure_input: String,
    /// The artifact a run uploads its JUnit files in.
    #[serde(default = "results")]
    pub results: String,
}

/// The fast workflow: dispatched by hand on a commit with the tests and checks to run, builds and runs only those.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiFast {
    /// The workflow, by its file's name, always run by its file on the main branch.
    pub workflow: String,
    /// Where it runs: `github`, GitHub's own machines, or the label of a self-hosted runner.
    pub runner: String,
    /// The input that takes the commit to check out and test.
    #[serde(default = "commit_input")]
    pub commit_input: String,
    /// The input that takes a value only one request knows, which the run's name must hold.
    #[serde(default = "call_input")]
    pub call_input: String,
    /// The input that takes the names of the tests to run, each followed by a line break.
    #[serde(default = "tests_input")]
    pub tests_input: String,
    /// The input that takes the checks to run beside the tests, a JSON array of their jobs' names.
    #[serde(default = "checks_input")]
    pub checks_input: String,
    /// The input that takes how long one test may run before it is stopped, in whole seconds; empty where the
    /// workflow takes none.
    #[serde(default = "test_stop_input")]
    pub test_stop_input: String,
    /// The checks it runs when named, each the name of a job of the workflow, such as the lint.
    #[serde(default)]
    pub checks: Vec<String>,
    /// The artifact a run uploads its JUnit files in.
    #[serde(default = "results")]
    pub results: String,
    /// How long a run may take, such as `2h`, `90m` or `30s`: one not complete by then is cancelled, and said as what
    /// it found.
    pub too_long: String,
}

/// The full workflows: every job required to merge, dispatched on the exact commit, run once before the merge.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiFull {
    /// The workflows, by their files' names, always run by their files on the main branch.
    pub workflows: Vec<String>,
    /// Where they run, as for the fast workflow.
    pub runner: String,
    /// How long a run may take, such as `2h`, `90m` or `30s`: one not complete by then is cancelled, and the check
    /// before the merge fails, its jobs not complete said so.
    pub too_long: String,
    /// The input that takes the commit to check out and test.
    #[serde(default = "commit_input")]
    pub commit_input: String,
    /// The input that takes a value only one request knows, which the run's name must hold.
    #[serde(default = "call_input")]
    pub call_input: String,
    /// The input that takes how long one test may run before it is stopped, in whole seconds; empty where the
    /// workflows take none.
    #[serde(default = "test_stop_input")]
    pub test_stop_input: String,
    /// The artifact a run uploads its JUnit files in.
    #[serde(default = "results")]
    pub results: String,
}

fn commit_input() -> String {
    "commit".to_string()
}

fn call_input() -> String {
    "call".to_string()
}

fn base_input() -> String {
    "base".to_string()
}

fn measure_input() -> String {
    "measure".to_string()
}

fn tests_input() -> String {
    "tests".to_string()
}

fn checks_input() -> String {
    "checks".to_string()
}

fn test_stop_input() -> String {
    "test_stop".to_string()
}

fn results() -> String {
    "test-results".to_string()
}

/// `agents/installation.toml`: the installation's settings the words sent to the agents may name, whose values the
/// engine fills in from the installation, read once for every project; and what a tool's result says where a work's item
/// belongs to no project the installation names.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub names: Vec<String>,
    /// What a tool's result says where the work's item belongs to no project the installation names.
    #[serde(default)]
    pub no_project: NoProject,
}

/// What a tool's result says where nothing is run, the work's item belonging to no project the installation names:
/// `unnamed` where it belongs to one the installation does not name, `{project}` its name; `none` where it belongs to
/// none.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoProject {
    pub unnamed: String,
    pub none: String,
}

/// A file of the project.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    /// Its path from the root of the project's checkout.
    pub path: String,
}

/// `agents/session.toml`: what a line's session begins with, its system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    /// Its parts, in the order they stand in it.
    pub parts: Vec<SessionPart>,
    /// How a file of the project stands in it: `{path}` its path, `{text}` its text.
    pub heading: String,
}

/// A part of the system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum SessionPart {
    Project(ProjectPart),
    Line(LinePart),
    Words(WordsPart),
    Returns(ReturnsPart),
}

impl SessionPart {
    /// The file of the project the part holds, by its name in the declaration.
    pub fn file(&self) -> Option<&str> {
        match self {
            SessionPart::Project(ProjectPart { project }) => Some(project),
            SessionPart::Line(_) | SessionPart::Words(_) | SessionPart::Returns(_) => None,
        }
    }
}

/// A file of the project, by its name in the declaration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPart {
    pub project: String,
}

/// What the line names for its own system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinePart {
    pub line: OfTheLine,
}

/// What of a line stands in its system prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfTheLine {
    /// The sets of rules the line takes, in the order it names them.
    Rules,
}

/// A text of the engine's, by its file in `agents/`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordsPart {
    pub words: String,
}

/// How a cell returns, made from `return/`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnsPart {
    pub returns: OfTheReturn,
}

/// What of the return stands in the system prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfTheReturn {
    /// How a cell returns, the same for every cell of the flow: `[how_you_return]` of `return.toml`, with each field
    /// and each tool the flow names.
    HowYouReturn,
}

impl OfTheReturn {
    /// Its name, as the files write it.
    pub fn name(self) -> &'static str {
        match self {
            OfTheReturn::HowYouReturn => "how_you_return",
        }
    }
}

/// [`YOUR_LINE_FILE`]: what the place `{your_line}` in the engine's words says the line may do in the worktree, as its
/// file says whether it only reads or changes only the project's tests.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YourLine {
    /// Of a line that only reads.
    pub reads_only: String,
    /// Of a line that writes.
    pub writes: String,
    pub or: String,
    /// Of a line that changes only the project's tests, after `writes`: `{paths}` the paths of the project's tests,
    /// each between backticks, joined by commas, and the last by `or`.
    pub tests_only: String,
    /// Of a line that writes, where the project protects paths, after the rest: `{paths}` those paths, each between
    /// backticks, joined by commas, and the last by `and`.
    pub protected: String,
    /// Of a line that only reads, where the project protects paths, after `reads_only`: `{paths}` as in `protected`.
    pub protected_read: String,
    pub and: String,
}

/// A set of rules for the agents as [`RULES_FILE`] names it, by its name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetFile {
    /// Its file, beside [`RULES_FILE`].
    pub file: String,
}

/// A set of rules for the agents, with its text.
#[derive(Clone, Debug)]
pub struct RuleSet {
    /// Its file, beside [`RULES_FILE`].
    pub file: String,
    /// The text of its file.
    pub text: String,
}

/// `agents/return/return.toml`: the form of a return, what the part on returning of a line's system prompt holds
/// together, and what a cell is told it writes, with its task.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Return {
    pub form: Form,
    pub how_you_return: Told,
    pub outputs: Outputs,
}

/// `agents/asks.toml`: the questions the engine itself asks the owner, by what it found of a work.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asks {
    pub protected: AskProtected,
}

/// The question the engine asks where a work's own change touches a path the project protects ([`Protected`]): asked
/// again under its name where that change is another, or the work gives another reason for it while it stands
/// unanswered, or it was answered with another option than `merges`; withdrawn once the work touches none. It waits
/// only where the check before the merge asks it, and recommends the option `merges` names. `ask` is what it asks,
/// `{files}` the files the change touches, each as `file` says, a blank line between two: `{path}` its path, `{why}`
/// why the work says the change is needed, or `no_reason` where it says nothing, and `{diff}` the file's change itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskProtected {
    pub name: String,
    pub ask: String,
    pub file: String,
    /// What stands for why, of a file the work changes and says nothing of.
    pub no_reason: String,
    pub options: Vec<AskOption>,
    /// The number of the option, counted from 1, by which the owner lets the change merge: the check before the merge
    /// passes a change under the protected paths only where its question was answered so.
    pub merges: u64,
    /// Why it is withdrawn, once the work's change touches no path the project protects.
    pub withdrawn: String,
}

/// An option of a question the engine asks: what the work does by it, and what follows from it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskOption {
    pub does: String,
    pub follows: String,
}

/// The form: the fields of the kind of return that does the cell's work, in their order, the same for every cell of
/// a line; the blocks of the line's cells go in the field [`OUTPUTS`]. The kind that asks for tools holds each tool
/// the flow names, and nothing else.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// The engine's fields a return that does the cell's work holds, by name, in order.
    pub outcome: Vec<String>,
}

/// What holds together the part on returning of a line's system prompt, the same for every cell of the flow.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Told {
    pub heading: String,
    pub first: String,
    /// The two kinds of return.
    pub kinds: String,
    /// An item of a list of fields or tools, `{text}` its words.
    pub item: String,
    /// The fields told `listed`, in order.
    pub listed: Vec<String>,
    pub fields: String,
    pub owner_reads: String,
    /// The fields the owner reads, in the order `owner_reads` names them.
    pub reads: Vec<String>,
    pub and: String,
    /// What heads the tools and says how the kind of return that asks for them holds them, where the flow names any.
    pub tools: String,
}

/// What a cell is told it writes, with its task: its blocks, each on a line after the heading.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outputs {
    pub heading: String,
    pub block: String,
    /// What a block's line says after its name: that it is a choice, `{values}` its values; that the owner reads it;
    /// that the cell must hand it in every time.
    pub choice: String,
    pub shown: String,
    pub must_write: String,
    /// What stands in place of the list where the cell writes no block.
    pub none: String,
    /// What follows the list, once, where a block of it is one the owner reads.
    pub owner_reads: String,
    /// What follows the list, once, where a block of it is one the cell must hand in every time.
    pub every_time: String,
}

/// A field of a return that is the engine's, which every agent cell has: `agents/return/fields/<name>.toml`. It is a
/// field of the kind of return that does the cell's work.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// Every return of that kind holds it.
    #[serde(default)]
    pub required: bool,
    pub told: ToldAs,
    pub words: String,
    /// What is told after the list of the fields, of a field told `listed`.
    pub also: Option<String>,
    pub shape: toml::Value,
}

/// A kind of return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnKind {
    /// A return that asks the engine for tools.
    Tools,
    /// A return that does the cell's work.
    Outcome,
}

impl ReturnKind {
    /// Its name, as the files write it, which is its field in a return.
    pub fn name(self) -> &'static str {
        match self {
            ReturnKind::Tools => "tools",
            ReturnKind::Outcome => "outcome",
        }
    }
}

/// `agents/return/sent_back.toml`: what a round is told of the return before it that was sent back, each thing wrong
/// with it on a line of its own, and nothing else.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentBack {
    /// What comes before the list.
    pub heading: String,
    /// What comes before the list of an earlier round of the run, told to a session that never saw it: `{round}` its
    /// number.
    pub earlier: String,
    /// An item of the list, `{text}` the words of one thing wrong.
    pub item: String,
    /// What comes between two names where a text names several.
    pub between: String,
    /// What follows the list.
    pub again: String,
    /// What follows the list where the return it is for stands for nothing of what the next gives: it asked for calls,
    /// or it could not be read, so the next gives it whole.
    pub again_whole: String,
    /// The words of each thing that can be wrong with a return.
    pub faults: SentBackFaults,
    /// Why what came back could not be read as a return, which [`SentBackFaults::unreadable`] says in its `{why}`.
    pub unread: Unread,
}

/// Why what came back could not be read as a return: `{error}` what the reading of its JSON says, `{name}` a tool's
/// name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unread {
    /// It does not have the form's shape.
    pub not_of_its_form: String,
    /// A tool's requests are not a list.
    pub requests_not_a_list: String,
}

/// The words of each thing that can be wrong with a return, by its kind. In a text, `{block}` is a block's name,
/// `{name}` a name as the return wrote it or, of what it dealt with, the datum's or the group's name, `{value}` a value it wrote,
/// `{values}` the values of a choice, each between backticks, `{version}` a version's number, `{why}` why what came
/// back could not be read and `{changes}` what git says the worktree holds that is not committed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SentBackFaults {
    pub asks_nothing: String,
    pub not_an_output: String,
    pub written_twice: String,
    pub no_value: String,
    pub not_a_value: String,
    pub not_a_choice: String,
    pub not_handed_in: String,
    /// `{block}` a choice written with `{value}`, which a return holds with nothing but what its file names, and
    /// `{besides}` the other blocks the return holds, each between backticks, joined by `between`.
    pub nothing_but: String,
    pub unreadable: String,
    pub not_run_by: String,
    pub dealt_with_twice: String,
    pub dealt_with_nothing: String,
    pub written_over: String,
    pub choice_changed: String,
    pub asks_and_works: String,
    pub not_watched: String,
    pub not_a_tool: String,
    pub unchanged_not_written: String,
    pub question_taken: String,
    pub no_such_question: String,
    pub question_twice: String,
    pub not_an_option: String,
    /// Of a return read before a cell could ask only where it writes the work's questions.
    pub not_answerable: String,
    pub not_answered: String,
    /// `{path}` a file the project protects that the work changes, and of which it says not why.
    pub unexplained: String,
    /// `{path}` a file that the run of a line that changes only the project's tests changed, and that is none of them.
    pub not_a_test: String,
    /// `{changes}` what git says the worktree holds of files it tracks that are not committed, where the round of a
    /// line that may change the work ended with a return that asks for no call.
    pub uncommitted: String,
    /// `{changes}` the same, where a merge is under way in the worktree, begun and not committed.
    pub merging: String,
    /// `{name}` a question of the work marked `tree`, with the owner's answer to its latest version, where the return
    /// writes a block handed in only with the answer in the design tree (`answers_in_the_tree`) and the work's change
    /// against main changes nothing in the tree.
    pub tree_unchanged: String,
    /// `{line}` a line of a block whose lines name entries of the design tree (`names_tree_entries`) that is not
    /// written as the tree's format (`agents/tree.toml`) refers to an entry, or names one the tree on the work's branch
    /// lacks.
    pub not_an_entry: String,
    /// `{faults}` each fault found in the forms of the design tree's files on the work's branch, a line to a fault, as
    /// `agents/tree.toml` words it, where the round's line may change the work, its return asks for no call, and the
    /// work's change against main changes the tree.
    pub tree_form: String,
    /// `{block}` what an act of the engine's wrote, dealt with by a return that leaves nothing new that runs the act.
    pub unanswered: String,
    /// `{name}` a dialogue the cell answers as the maker, named as dealt with and answered in nothing, and `{watched}`
    /// what the other side watches, each between backticks, joined by `between` (design document `data-and-context`,
    /// 42).
    pub word_owed: String,
    /// `{block}` a verdict written with `{value}`, which does not pass, and no version of its dialogue, `{name}`.
    pub no_point: String,
}

impl SentBackFaults {
    /// The words of the thing wrong that `agents/return/sent_back.toml` keeps under this key, as they are written, with
    /// no place filled; none where it keeps none.
    pub fn text(&self, key: &str) -> Option<String> {
        let fields = serde_json::to_value(self).ok()?;
        fields.get(key)?.as_str().map(str::to_string)
    }
}

/// Where a field's words go in the part on returning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToldAs {
    /// A paragraph of its own.
    Alone,
    /// After the words of [`OUTPUTS`], in their paragraph.
    WithOutputs,
    /// In the list of the fields that are not blocks.
    Listed,
}

/// A tool of the engine's: `agents/tools/<name>.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub words: String,
    /// The names its words use that the tool itself defines, such as the keys of what it gives and the SQL function it
    /// points to: the Flow page counts them as defined among the names between backticks in the texts sent to agents.
    /// Nothing is filled in for them, as for the settings that `names` in `agents/installation.toml` lists.
    #[serde(default)]
    pub defines: Vec<String>,
    /// What the tool says of its own answers, by name, `{name}` places in each filled where it is said.
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
    /// How many bytes of a result of the tool `query` a message gives; a result longer is given cut and kept whole in
    /// the records and in a file. Only `query` has its results cut. With none, a result is given whole.
    pub given_bytes: Option<usize>,
    /// The shape of a request of it.
    pub request: toml::Value,
    /// What fills each place of its words that the engine fills by how the installation runs the tool, by the place's
    /// name: the words for each way, by the way's name, which the engine chooses.
    #[serde(default)]
    pub places: BTreeMap<String, BTreeMap<String, String>>,
    /// What a round is told of a request of it that cannot be read as that shape: `{error}` what the reading said.
    pub unread: Option<String>,
    /// What a round is told where the tool could not run its check at all, whatever the host: `{why}` what stopped it.
    pub could_not_check: Option<String>,
    /// How much of a result of it a round is given.
    pub given: Option<ResultGiven>,
    /// What it says where the host is the local repository, in place of `words`.
    pub words_local: Option<String>,
    /// What it says of what a run found, whatever the host.
    pub result: Option<ResultWords>,
    /// What it says where the host is GitHub.
    pub github: Option<GithubWords>,
    /// What it says where the host is the local repository.
    pub local: Option<LocalWords>,
}

/// What a result of `run_tests` says of the tests and checks a run found. `{...}` are filled in a run.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultWords {
    /// What opens it: `{which}` the workflow's role (`which_fast`, `which_full`), `{commit}`, `{url}` of the run,
    /// `{came}` what it came to.
    pub head: String,
    pub which_fast: String,
    pub which_full: String,
    pub which_measure: String,
    /// What it came to: `{passed}` of `{total}`, tests, measurements and checks, apart by `came_join`.
    pub came_tests: String,
    pub came_measures: String,
    pub came_checks: String,
    pub came_join: String,
    /// The title of the tests that did not pass, of those that passed, of the timings of the measurements that ran, and of
    /// the jobs.
    pub tests_title: String,
    pub passed_title: String,
    pub timings_title: String,
    pub jobs_title: String,
    /// A test: `{name}`, `{became}` one of the following, `{time}` as `time` says it where the results give one; the
    /// time of a job is said the same way.
    pub test: String,
    /// A case of a measurement asked for: `{name}`, `{unit}` what was timed, `{became}`, `{time}` as for `test`.
    pub measure_case: String,
    pub time: String,
    pub passed: String,
    pub failed: String,
    pub skipped: String,
    /// A name asked for that no test of the run has: `{why}`.
    pub not_run: String,
    /// A name asked for, no test of the run has it, and a unit was not built: `{why}`.
    pub not_built: String,
    /// What a test that failed said, as the results hold it: `{said}`.
    pub output: String,
    /// Said beneath the line of a test that failed and is red on the main of the project as well, as the engine knows
    /// it from the checks it ran on main's head: `{main}` the main branch's name, `{head}` the head it failed on.
    pub also_on_main: String,
    /// A job of the workflow: `{name}`, `{how}` one of the following, `{time}` as `time` says it for a job that ran to a
    /// conclusion, or as `job_untimed` says where GitHub gives no time for it, and `{link}` as `link` says it.
    pub job: String,
    pub link: String,
    pub job_passed: String,
    pub job_failed: String,
    pub job_broken: String,
    pub job_pending: String,
    /// A job that did not run.
    pub job_skipped: String,
    /// A job that ran to a conclusion and has no time to give.
    pub job_untimed: String,
    /// A check named that the run has no job of.
    pub job_missing: String,
    /// What is said beneath a job that failed with no failed test of its own to say why: `{said}` the end of what the
    /// step that failed it printed, as its log holds it; or, where its log could not be read, `{why}`.
    pub step_end: String,
    pub log_unread: String,
    /// The run uploaded no JUnit it can read, so no test's result is known: `{why}`.
    pub no_junit: String,
    /// What the check before a merge was judged by: `{urls}` the runs.
    pub verdict_by: String,
    /// What the check before a merge says of the jobs of its runs that are not said as what failed: `{jobs}` each as
    /// a line of `job`, joined by a line break and the indent that sets them under it.
    pub verdict_jobs: String,
    /// A unit not built: `{unit}`, `{why}`.
    pub unit_not_built: String,
    /// A name asked for that no test of the run has; and the same where a unit was not built, so that the test may be
    /// in it: `{units}` them.
    pub no_such_test: String,
    pub no_such_test_unbuilt: String,
    /// Each test a name has was skipped.
    pub all_skipped: String,
}

/// What `run_tests` says where the host is GitHub.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubWords {
    /// A merge is under way in the worktree, and no commit is named: `{changes}`.
    pub merge_under_way: String,
    /// The worktree has changes to files git tracks that are not committed, and no commit is named: `{changes}`.
    pub uncommitted: String,
    /// The worktree holds files git does not track, which are not in the run, and no commit is named: `{files}` one to
    /// a line, a directory holding only such files as its name with a slash. Said before the result.
    pub untracked: String,
    /// Neither a test nor a check is named, so nothing is run.
    pub none_named: String,
    /// The commit named is none git finds in the worktree: `{why}`.
    pub commit_unknown: String,
    /// Tests or checks are named and the project declares no workflows to run them by.
    pub no_fast: String,
    /// Checks named that the fast workflow does not run by name: `{unknown}`, `{runs}` what it runs.
    pub unknown_checks: String,
    /// The workflow's runner is not one this installation has: `{runner}`, `{have}`.
    pub runner_missing: String,
    /// Measurements are named and the project declares no measure workflow.
    pub no_measure: String,
    /// The commit is left on GitHub: `{commit}`, and `{branch}` the branch it is on. Said before what the request ran.
    pub published: String,
}

/// What `run_tests` says where the host is the local repository.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalWords {
    /// What the project's command said: `{command}`, `{outcome}`, `{said}`.
    pub ran: String,
    pub passed: String,
    /// `{status}` how it ended.
    pub failed: String,
    /// It could not be run: `{command}`, `{why}`.
    pub could_not: String,
    /// Said before what it found where tests or checks are named, which are not run apart.
    pub not_apart: String,
    /// A commit is named, and the command runs on the worktree as it is: nothing was run.
    pub no_commit: String,
    /// Measurements are named, and the command runs the tests alone: nothing was run.
    pub no_measure: String,
    /// A commit is to be published, and there is no host to leave it on: nothing was run.
    pub no_publish: String,
}

/// How much of a tool's result a round is given: the result is kept whole, and given cut where it is longer.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultGiven {
    /// The most bytes of a result a round is given.
    pub bound: usize,
    /// What follows a result cut: `{given}` the bytes of it given, `{size}` the bytes it is whole, `{file}` the file
    /// that holds it whole.
    pub cut: String,
}

impl ResultGiven {
    /// A result as it is given: its end of whitespace left off, and cut at the most bytes `bound` allows, at a whole
    /// character, with what `note` says after it, given the bytes of the result kept and the bytes it is whole; as it
    /// is where it is no longer than the bound. Whoever gives a result, a round or a datum, gives it this way.
    pub fn cut_with(&self, result: &str, note: impl FnOnce(&str, &str) -> String) -> String {
        if result.len() <= self.bound {
            return result.trim_end().to_string();
        }
        let mut at = self.bound;
        while !result.is_char_boundary(at) {
            at -= 1;
        }
        let (kept, size) = (at.to_string(), result.len().to_string());
        format!("{}\n\n{}", &result[..at], note(&kept, &size))
    }
}

/// `agents/message/message.toml`: what the engine sends a cell's agent as it enters the cell, its parts in order.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub parts: Vec<MessagePart>,
    /// What a session compacted during a round is told before the round's message is given again, whole.
    pub compacted: String,
    /// What the owner says to a round while it runs, as its session is given it after its next use of a tool: `{entry}`
    /// the owner's version of the cell's words, `{block}` their name, `{number}` the version's place, `{do}` what the
    /// cell does when its words are new.
    pub said: String,
    /// What the owner says, or answers, to a cell before the one whose round is under way, as its session is given it
    /// after its next use of a tool, telling it to hand in what it has now: `{entry}` what was said, `{block}` the
    /// datum's name, `{number}` the version's place.
    pub hurried: String,
}

/// A part of a message.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagePart {
    /// What the part is.
    pub part: Part,
    /// Of a part the session keeps, as data are kept, the kind it is kept in, as a datum's: it is sent where the
    /// session lacks its latest version, and again when it changes. A part with none is written anew each time.
    pub kind: Option<InputKind>,
    /// Of a part written anew each time, when it is sent at all; each time where none.
    pub only: Option<Only>,
    pub text: Option<String>,
    /// Of the part `main`, what it says in place of `text` where main was not merged into the work's branch: for a
    /// conflict, `{files}` the files in conflict; for changes not committed or a merge under way, `{changes}` the files
    /// the worktree holds changes of; and for any other cause, nothing but that it was not. In each, `{name}` is the
    /// main branch's name and `{main}` its head.
    pub conflict: Option<String>,
    pub uncommitted: Option<String>,
    pub merging: Option<String>,
    pub failed: Option<String>,
    /// Of a part that is a list, each item of it.
    pub each: Option<String>,
    /// Of a part that is a list, or that names one, as the files in conflict: what comes between two of its items.
    pub between: Option<String>,
    /// What is sent in its place where the session holds it already.
    pub held: Option<String>,
    /// Of what is new to the cell: what a datum's line says where the cell worked from a version before, `{last}` that
    /// version's place (design document `data-and-context`, 10 and 11).
    pub since: Option<String>,
    /// Of what is new to the cell: what a datum's line says of the one version that changed its meaning, `{changed}`
    /// its place; none is said where none did.
    pub changed_one: Option<String>,
    /// And of several, `{changed}` their places, joined by `between`.
    pub changed_many: Option<String>,
    /// Of what is new to the cell: what a datum's line says the cell does, `{do}` its words.
    #[serde(rename = "do")]
    pub does: Option<String>,
    /// What follows the list.
    pub after: Option<String>,
    /// Of the part `context`, the order of what a round is given, by where the cell's definition gives it: each list
    /// named once, each in the order it names its data (design document `data-and-context`, 14, 15, 21).
    pub order: Option<Vec<Listed>>,
}

/// What a part of a message is: what the engine puts into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    /// What the last return handed in is kept as.
    Taken,
    /// The cell's title.
    Heading,
    /// Which work and which item the session is for.
    Work,
    /// The commit of main taken in, or why it could not be.
    Main,
    /// Each datum of the cell's context the session lacks or that is new to the cell.
    Context,
    /// The blocks the cell writes, as `[outputs]` of `return.toml` says.
    Outputs,
    /// The cell's task.
    Task,
    /// The results of the calls the round is given, where its session lacks them.
    Results,
    /// Why what a round of the run returned was sent back, where its session lacks it.
    SentBack,
    /// Each datum or group the cell watches that is new to it.
    New,
}

impl Part {
    /// Its name, as the files write it.
    pub fn name(self) -> &'static str {
        match self {
            Part::Taken => "taken",
            Part::Heading => "heading",
            Part::Work => "work",
            Part::Main => "main",
            Part::Context => "context",
            Part::Outputs => "outputs",
            Part::Task => "task",
            Part::Results => "results",
            Part::SentBack => "sent_back",
            Part::New => "new",
        }
    }
}

/// When a part written anew each time is sent at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Only {
    /// After a return that handed in blocks.
    AfterHandIn,
    /// To a cell that takes main in.
    TakesMain,
}

impl Only {
    /// Its name, as the files write it.
    pub fn name(self) -> &'static str {
        match self {
            Only::AfterHandIn => "after_hand_in",
            Only::TakesMain => "takes_main",
        }
    }
}

/// `agents/message/given.toml`: how a datum a cell is given is put into its session, by its kind.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Given {
    /// What a text says of a datum that is a choice.
    pub choice: String,
    /// What stands in place of a datum's content where the session holds its latest version already: `held` where the
    /// cell worked from an earlier version, `held_new` where it never worked from it.
    pub held: String,
    pub held_new: String,
    /// Who wrote a version, as a round is told it at `{by}` in each heading of a version: the line of the cell whose
    /// run wrote it, `{line}`; or the owner. Nothing where neither did (design document `data-and-context`, 29).
    pub by_line: String,
    pub by_owner: String,
    /// How a block whose lines name entries of the design tree (`names_tree_entries`) is given in place of its names.
    pub entries: Entries,
    pub latest: GivenKind,
    pub last_and_latest: GivenKind,
    pub all: GivenKind,
}

/// How the entries a block names are given: each file's entries under `file`, `{path}` the file's path, and an entry
/// named that the tree's latest version does not hold as `gone`, `{reference}` its line, after the files.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entries {
    pub file: String,
    pub gone: String,
}

/// How a datum given in one kind is put in.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GivenKind {
    pub text: String,
    /// What is sent in place of `text` where the session lacks what it needs, or `text` would be longer.
    pub whole: Option<String>,
    /// Of a datum whose versions the engine compares itself, as the design tree, what is sent in place of `text` where
    /// the session holds none of its versions: its latest whole, saying so (design document `data-and-context`, 23).
    pub unread: Option<String>,
}

/// `agents/message/results.toml`: what a round is told of the results of the calls it is given. In a text, `{name}` is
/// a tool's name, `{input}` what it was run on, `{call}` a call's number.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Results {
    pub heading: String,
    /// A tool run on an input.
    pub tool: String,
    /// A tool run on nothing.
    pub tool_alone: String,
    /// The owner's word at a limit.
    pub at_limit: String,
    /// A call the work does not hold.
    pub unknown: String,
    /// What stands in place of a result where a call has none.
    pub none: String,
    /// What follows the results.
    pub after: String,
}

/// What the engine writes as it works: into the blocks its acts write, and in the commits it makes.
#[derive(Clone, Debug)]
pub struct Writes {
    pub acts: Acts,
    pub commits: Commits,
}

/// `agents/commits.toml`: who the engine's commits are by, and the words of each.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commits {
    pub name: String,
    pub email: String,
    /// A run's merge of main into the work's branch.
    pub run: String,
    /// The check's merge of main into the work's branch.
    pub check: String,
    /// The merge of the work's branch into main.
    pub merge: String,
    /// The name of a work's merge, where the work has written none.
    pub unnamed: String,
    /// The line that names the item, which every commit of a work ends with: `{item_number}` its number alone.
    pub item_line: String,
    /// The line that names the work, which every commit of a work ends with: `{work}` its number as it is written.
    pub work_line: String,
}

/// The place in `item_line` that the engine fills with the item's number alone: no other text of the engine's writes an
/// item's number without its `#`, so no place of [`NUMBER_PLACES`] is it.
const ITEM_NUMBER: &str = "{item_number}";

/// The place in `work_line` that the engine fills with the work's number as it is written.
const WORK_NUMBER: &str = "{work}";

impl Commits {
    /// The line that names the item and the line that names the work, given the numbers as the engine writes each: the
    /// item's alone and the work's as everywhere.
    pub fn lines(&self, item: &str, work: &str) -> (String, String) {
        (
            fill(&self.item_line, &[("item_number", item)]),
            fill(&self.work_line, &[("work", work)]),
        )
    }

    /// The message with the two lines at its end, set apart from it by a blank line: what every commit the engine makes
    /// ends with, as the hook gives the agents' commits the same.
    pub fn ending(&self, message: &str, item: &str, work: &str) -> String {
        let (item_line, work_line) = self.lines(item, work);
        format!("{}\n\n{item_line}\n{work_line}", message.trim_end())
    }

    /// Whether the line is the line that names an item or the line that names a work, of any item or any work: a
    /// commit copied from another work ends with the lines of that one. What stands in the place of the number is the
    /// number of an item where `item` says so, and of a work where `work` says so: how each is written is not the
    /// words' to say.
    pub fn is_one_of_the_lines(&self, line: &str, item: impl Fn(&str) -> bool, work: impl Fn(&str) -> bool) -> bool {
        let is = |form: &String, place: &str, number: &dyn Fn(&str) -> bool| {
            let Some((before, after)) = form.split_once(place) else {
                return false;
            };
            (line.strip_prefix(before))
                .and_then(|rest| rest.strip_suffix(after))
                .is_some_and(number)
        };
        is(&self.item_line, ITEM_NUMBER, &item) || is(&self.work_line, WORK_NUMBER, &work)
    }

    /// What is wrong with the two lines, as one sentence for each: each must name its number, or every commit would
    /// end with a line that names nothing, and be one line, or the trailer block would hold a stray line.
    fn faults(&self) -> Vec<String> {
        let mut faults = Vec::new();
        for (key, line, place) in [
            ("item_line", &self.item_line, ITEM_NUMBER),
            ("work_line", &self.work_line, WORK_NUMBER),
        ] {
            if !line.contains(place) {
                faults.push(format!("{key} does not name {place}"));
            }
            if line.contains('\n') {
                faults.push(format!("{key} is not one line"));
            }
        }
        faults
    }
}

/// What the engine's acts write into the blocks their cells write: `agents/acts/<act>.toml`; and the tree's rules the
/// check before the merge holds the work to, which the rounds of the lines hold it to as well: `agents/tree.toml`.
#[derive(Clone, Debug)]
pub struct Acts {
    pub start: Start,
    pub check: Check,
    pub merge: Merge,
    pub deliver: Deliver,
    pub conclude: Conclude,
    pub tree: Tree,
}

/// `agents/acts/start.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub text: String,
    /// What it writes where the work's item continues a branch or a pull request.
    pub continued: String,
}

/// `agents/acts/merge.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub text: String,
    pub moved: String,
    /// The work's head moved since the check passed: the merge fails in passing, and the review and the check run first.
    pub unreviewed: String,
}

/// `agents/acts/deliver.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deliver {
    pub none: String,
    pub one: String,
    pub many: String,
    pub between: String,
}

/// `agents/acts/conclude.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conclude {
    pub text: String,
}

/// `agents/acts/check.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub first: String,
    pub checked: String,
    pub by: String,
    pub passed: String,
    pub failed: String,
    pub why: String,
    /// What fails the work fails on the head of main as well: the choice alone is written, and the work waits for main.
    pub held: String,
    pub took_in: String,
    pub notes: String,
    pub note: String,
    /// The project declares no full workflows for the check to run: nothing was run.
    pub no_full: String,
    /// What the check before a merge says where its full workflows failed tests alone, which it runs again by the fast
    /// workflow: why they were not run again, `{why}`; what the run on the commit found, `{run}` as a result says it;
    /// what the run on the head of main found of what failed again, `{main}` the main branch's name, `{head}` its head,
    /// `{run}`; a test that failed and then passed on the commit, `{test}`, `{commit}`, `{failed}` the links of the
    /// full workflows' runs and `{again}` of the run again; the tests that failed again and not on main, and those that
    /// failed on main as well, `{tests}` each as `test_named` says it, `{test}` its name, joined by `tests_between`.
    pub again_not_run: String,
    pub again_on_commit: String,
    pub again_on_main: String,
    pub flaky: String,
    pub only_here: String,
    pub on_main_too: String,
    pub test_named: String,
    pub tests_between: String,
    pub values: CheckValues,
    pub found: Found,
    pub refused: CheckRefused,
}

/// The values the check writes in its choice.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckValues {
    pub passed: String,
    pub failed: String,
}

/// What the check finds before the host is asked.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Found {
    pub conflict: String,
    /// Questions of the work marked `tree` have the owner's answer to their latest version, and the work's change
    /// against main changes nothing in the design tree: `{questions}` their names, one to a line, and `{main}` the main
    /// branch's name. Said after `conflict` where both are found, a blank line between.
    pub tree_unchanged: String,
    /// The commit that would merge, main taken in, has these faults in the forms of the design tree's files, where the
    /// work's own change against main changes the tree: `{faults}` each, a line to a fault, as `agents/tree.toml` words
    /// it.
    pub tree_form: String,
    /// The work's own change touches paths the project protects, and the owner has not let it merge.
    pub protected: String,
    /// The change under the paths the project protects could not be looked at.
    pub protected_unread: String,
    /// The work's head is not one the review dealt with: the check fails in passing, and the review runs first.
    pub unreviewed: String,
    /// The commit checked changes nothing against main: there is nothing to merge.
    pub empty: String,
}

/// What the check says where it cannot go on: shown to the owner, whom the work waits for, and written into no block.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRefused {
    /// A merge begun in the worktree is not committed, and the engine takes back no merge it did not begin: `{main}` the
    /// main branch's name, `{changes}` what git says the worktree holds uncommitted.
    pub merging: String,
}

/// A piece of a text of the engine's words: what is sent as written, or a place filled in a run, by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    Text(&'a str),
    Place(&'a str),
}

/// A text cut into what is sent as written and its places. A place is `{name}`, its name small letters and
/// underscores. `{{`, to what closes it with `}}`, is sent as written: it is how a text points to a question by its
/// name. A brace that opens no place is sent as written. Every reader of the engine's words finds the places so.
pub fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    // Where the text sent as written that is not yet a piece begins.
    let mut from = 0;
    let mut at = 0;
    while let Some(open) = text[at..].find('{').map(|o| at + o) {
        if text[open..].starts_with("{{") {
            at = text[open..].find("}}").map_or(text.len(), |close| open + close + 2);
            continue;
        }
        let after = &text[open + 1..];
        match after.find('}').map(|close| &after[..close]) {
            Some(name) if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') => {
                if open > from {
                    pieces.push(Piece::Text(&text[from..open]));
                }
                pieces.push(Piece::Place(name));
                from = open + name.len() + 2;
                at = from;
            }
            _ => at = open + 1,
        }
    }
    if text.len() > from {
        pieces.push(Piece::Text(&text[from..]));
    }
    pieces
}

/// A text as a sentence: as it is where it ends as a sentence does, in English or in Chinese, else with a full stop; so
/// that reasons said one after another each read as one.
pub fn sentence(text: &str) -> String {
    let text = text.trim_end();
    match text.is_empty() || text.ends_with(['.', '?', '!', '。', '？', '！']) {
        true => text.to_string(),
        false => format!("{text}."),
    }
}

/// A text with each place these give filled by its value; a place they do not give stays as it is written. What fills
/// a place is sent as it is: a place in it is not filled again.
pub fn fill(text: &str, slots: &[(&str, &str)]) -> String {
    (pieces(text).into_iter())
        .map(|piece| match piece {
            Piece::Text(text) => text.to_string(),
            Piece::Place(name) => match slots.iter().find(|(n, _)| *n == name) {
                Some((_, value)) => value.to_string(),
                None => format!("{{{name}}}"),
            },
        })
        .collect()
}

/// Reads it all from the directory that holds the flows, anew each time, so that an edit of a file shows; or says which
/// file cannot be read, and why. A name in the form or in `how_you_return` that is no field of `return/fields/` is
/// refused as well, on the file that names them.
pub fn read(root: &Path) -> Result<Agents, String> {
    let at = root.join(AGENTS_DIR);
    let session_file = at.join("session.toml");
    let session: Session = file(&session_file)?;
    let mut words = BTreeMap::new();
    for part in &session.parts {
        if let SessionPart::Words(WordsPart { words: name }) = part {
            words.insert(name.clone(), text(&at.join(name))?);
        }
    }
    let rules_file = root.join(RULES_FILE);
    let sets: BTreeMap<String, RuleSetFile> = file(&rules_file)?;
    let beside = rules_file.parent().unwrap_or(root);
    let mut rules = BTreeMap::new();
    for (name, set) in sets {
        let text = text(&beside.join(&set.file))?;
        rules.insert(name, RuleSet { file: set.file, text });
    }
    let return_file = at.join("return").join("return.toml");
    let returns: Return = file(&return_file)?;
    let fields: BTreeMap<String, Field> = each(&at.join("return").join("fields"))?;
    let named = (returns.form.outcome.iter())
        .chain(&returns.how_you_return.listed)
        .chain(&returns.how_you_return.reads);
    for name in named {
        if !fields.contains_key(name) {
            let what = format!("names the field {name}, which return/fields/ does not have");
            return Err(format!("{}: {what}", return_file.display()));
        }
    }
    let tools: BTreeMap<String, Tool> = each(&at.join("tools"))?;
    for (name, known) in TOOL_TEXTS {
        let Some(tool) = tools.get(*name) else { continue };
        let file = at.join("tools").join(format!("{name}.toml"));
        let lacks: Vec<&str> = known.iter().copied().filter(|k| !tool.texts.contains_key(*k)).collect();
        let more: Vec<&str> = (tool.texts.keys().map(String::as_str))
            .filter(|k| !known.contains(k))
            .collect();
        if !lacks.is_empty() || !more.is_empty() {
            let (lacks, more) = (lacks.join(", "), more.join(", "));
            let what = format!("[texts] lacks: {lacks}; has what the engine does not say: {more}");
            return Err(format!("{}: {what}", file.display()));
        }
    }
    let message_file = at.join("message").join("message.toml");
    let message: Message = file(&message_file)?;
    for part in &message.parts {
        let order = part.order.as_deref().unwrap_or_default();
        let wrong = match part.part {
            Part::Context => {
                let named = |list: Listed| order.iter().filter(|l| **l == list).count();
                let lists: Vec<&str> = (Listed::ALL.into_iter())
                    .filter(|l| named(*l) != 1)
                    .map(Listed::name)
                    .collect();
                (!lists.is_empty()).then(|| {
                    format!(
                        "the part context names in `order` not once each of: {}",
                        lists.join(", ")
                    )
                })
            }
            _ => part
                .order
                .is_some()
                .then(|| format!("the part {} has an `order`, which only context has", part.part.name())),
        };
        if let Some(wrong) = wrong {
            return Err(format!("{}: {wrong}", message_file.display()));
        }
    }
    let writes = writes(root)?;
    Ok(Agents {
        root: root.to_path_buf(),
        session,
        words,
        your_line: file(&at.join(YOUR_LINE_FILE))?,
        installation: file(&at.join(INSTALLATION_FILE))?,
        refused: refused(&at.join(REFUSED_FILE))?,
        allowed: allowed(&at.join(ALLOWED_FILE))?,
        rules,
        returns,
        sent_back: file(&at.join("return").join("sent_back.toml"))?,
        fields,
        tools,
        message,
        given: file(&at.join("message").join("given.toml"))?,
        results: file(&at.join("message").join("results.toml"))?,
        acts: writes.acts,
        commits: writes.commits,
        asks: asks(&at.join(ASKS_FILE))?,
        cost: cost(&at.join("cost.toml"))?,
        pull_request: pull_request(&at.join(PULL_REQUEST_FILE))?,
        calls: calls(&at.join(CALLS_FILE))?,
    })
}

/// The texts a file gives, each of which must say something: a file whose text is empty would have the engine say
/// nothing where it speaks, which is refused (principle nothing-passes-for-normal).
fn filled(path: &Path, texts: &[(&str, &str)]) -> Result<(), String> {
    let empty: Vec<&str> = (texts.iter())
        .filter(|(_, text)| text.trim().is_empty())
        .map(|(key, _)| *key)
        .collect();
    match empty.is_empty() {
        true => Ok(()),
        false => Err(format!("{}: {} must not be empty", path.display(), empty.join(", "))),
    }
}

/// What the engine says of the agents' program, its rounds' cost and their sessions' caches, read from its file.
fn cost(path: &Path) -> Result<Cost, String> {
    let cost: Cost = file(path)?;
    let (program, check, work) = (&cost.program, &cost.check, &cost.work);
    filled(
        path,
        &[
            ("program.no_stream_result", &program.no_stream_result),
            ("program.unreadable_result", &program.unreadable_result),
            ("program.no_return", &program.no_return),
            ("program.error_kind", &program.error_kind),
            ("program.error", &program.error),
            ("program.last_attempt", &program.last_attempt),
            ("program.refused", &program.refused),
            ("program.too_long", &program.too_long),
            ("program.lines_too_long", &program.lines_too_long),
            ("program.not_a_result", &program.not_a_result),
            ("program.no_reason", &program.no_reason),
            ("check.request", &check.request),
            ("work.failed", &work.failed),
        ],
    )?;
    Ok(cost)
}

/// What the engine writes on a work's pull request, read from its file.
fn pull_request(path: &Path) -> Result<PullRequest, String> {
    let words: PullRequest = file(path)?;
    filled(
        path,
        &[
            ("about", &words.about),
            ("body", &words.body),
            ("cut", &words.cut),
            ("merged", &words.merged),
            ("closed", &words.closed),
            ("superseded", &words.superseded),
            ("withheld.data", &words.withheld.data),
            ("withheld.repository", &words.withheld.repository),
            ("withheld.home", &words.withheld.home),
        ],
    )?;
    Ok(words)
}

/// The result a call has where the engine ends it itself, read from its file.
fn calls(path: &Path) -> Result<Calls, String> {
    let words: Calls = file(path)?;
    filled(
        path,
        &[
            ("withdrawn", &words.withdrawn),
            ("waited", &words.waited),
            ("no_such_tool", &words.no_such_tool),
        ],
    )?;
    Ok(words)
}

/// What every agent is refused, read from its file, whose entries hold no slot but [`PROGRAM_SLOT`]: any other `{...}`
/// would be given to the program as it is.
fn refused(path: &Path) -> Result<Commands, String> {
    let refused: Commands = file(path)?;
    for entry in &refused.commands {
        let rest = entry.replace(PROGRAM_SLOT, "");
        if rest.contains(['{', '}']) {
            let what = format!("{entry:?} holds a slot; the only slot is {PROGRAM_SLOT}");
            return Err(format!("{}: {what}", path.display()));
        }
    }
    Ok(refused)
}

/// What every agent may do, read from its file, each word once.
fn allowed(path: &Path) -> Result<Allowed, String> {
    let allowed: Allowed = file(path)?;
    for (i, ability) in allowed.may.iter().enumerate() {
        if allowed.may[..i].contains(ability) {
            let what = format!("`may` names {} twice", ability.word());
            return Err(format!("{}: {what}", path.display()));
        }
    }
    Ok(allowed)
}

/// The questions the engine asks, read from their file, whose option that lets a change merge must be one of the
/// question's options.
fn asks(path: &Path) -> Result<Asks, String> {
    let asks: Asks = file(path)?;
    let protected = &asks.protected;
    let count = protected.options.len() as u64;
    if !(1..=count).contains(&protected.merges) {
        let what = format!(
            "[protected] merges is {}, which is not the number of one of its {count} options",
            protected.merges
        );
        return Err(format!("{}: {what}", path.display()));
    }
    Ok(asks)
}

/// What the engine writes as it works, read from the directory that holds the flows, as [`read`] reads it.
pub fn writes(root: &Path) -> Result<Writes, String> {
    let at = root.join(AGENTS_DIR);
    Ok(Writes {
        acts: acts(root)?,
        commits: commits(&at.join(COMMITS_FILE))?,
    })
}

/// `agents/commits.toml`, read, and refused where a line it gives names no number.
fn commits(path: &Path) -> Result<Commits, String> {
    let commits: Commits = file(path)?;
    let faults = commits.faults();
    if !faults.is_empty() {
        return Err(format!("{}: {}", path.display(), faults.join("; ")));
    }
    Ok(commits)
}

/// The acts' words and the tree's rules alone, read from the directory that holds the flows, as [`read`] reads them.
pub fn acts(root: &Path) -> Result<Acts, String> {
    let tree_file = root.join(AGENTS_DIR).join(TREE_FILE);
    let tree: Tree = file(&tree_file)?;
    let faults = tree.faults();
    if !faults.is_empty() {
        return Err(format!("{}: {}", tree_file.display(), faults.join("; ")));
    }
    let at = root.join(AGENTS_DIR).join(ACTS_DIR);
    Ok(Acts {
        start: file(&at.join("start.toml"))?,
        check: file(&at.join("check.toml"))?,
        merge: file(&at.join("merge.toml"))?,
        deliver: file(&at.join("deliver.toml"))?,
        conclude: file(&at.join("conclude.toml"))?,
        tree,
    })
}

/// The text of a file.
fn text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A file read into its shape.
fn file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    toml::from_str(&text(path)?).map_err(|e| format!("{}: {}", path.display(), e.to_string().trim_end()))
}

/// Each `.toml` file directly in a directory, read into its shape, by its name without `.toml`.
fn each<T: DeserializeOwned>(dir: &Path) -> Result<BTreeMap<String, T>, String> {
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = (path.file_name().and_then(|n| n.to_str())).and_then(|n| n.strip_suffix(".toml")) else {
            continue;
        };
        out.insert(name.to_string(), file(&path)?);
    }
    Ok(out)
}
