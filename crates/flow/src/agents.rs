//! What the engine says to every agent, whatever the flow, as it is written beside the flows: the project's files and
//! settings (`project.toml`), and in `agents/` what a session begins with, the rules for the agents, the engine's own
//! words, the return and its fields, the tools, the message a cell is sent as it is entered (design document
//! `data-and-context`, 51), and what the engine writes as it works: into the blocks its acts write, and in the commits
//! it makes. Every word an agent is sent that is not a flow's is in these files; the code holds none of them, only how
//! they are put together. A field a file does not know is refused, and what cannot be read is said with the path of
//! its file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::InputKind;

/// The directory beside the flows that holds what the engine says to every agent.
pub const AGENTS_DIR: &str = "agents";

/// The file beside the flows that names the project's files and settings.
pub const PROJECT_FILE: &str = "project.toml";

/// A project's declaration, the same file as [`PROJECT_FILE`], by its path from the root of the project's repository:
/// each project an installation names declares its files, its settings, its tests and its workflows there, and the
/// works of its items are run with it in place of the one beside the flows.
pub const DECLARATION: &str = "flow/project.toml";

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

/// The file, in `agents/`, of what the place `{your_line}` in the engine's words says: what the line may do in the
/// worktree.
pub const YOUR_LINE_FILE: &str = "your_line.toml";

/// The texts the engine fills of a tool, by the tool: its file holds each of these and no other, since the engine says
/// each of them in some answer and a misspelt name would leave an answer empty. A tool not named here has none.
pub const TOOL_TEXTS: &[(&str, &[&str])] = &[(
    "query",
    &[
        "closed",
        "unreadable",
        "error",
        "timeout",
        "unavailable",
        "row_too_long",
        "hides_nothing",
        "store_table",
        "store_column",
        "store_column_commented",
    ],
)];

/// The file, in `agents/`, of the questions the engine itself asks the owner.
pub const ASKS_FILE: &str = "asks.toml";

/// The field of a return that holds the blocks a cell writes, each under its name. The form does not list them: the
/// cell is told its own with its task, and the engine checks what it writes against them.
pub const OUTPUTS: &str = "outputs";

/// All of it, read from the directory that holds the flows.
#[derive(Clone, Debug)]
pub struct Agents {
    /// The directory read, which holds the flows.
    pub root: PathBuf,
    pub project: Project,
    pub session: Session,
    /// The text of each file of the engine's own words that the session names, by its name in `agents/`.
    pub words: BTreeMap<String, String>,
    pub your_line: YourLine,
    /// What the texts take from the installation: `agents/installation.toml`.
    pub installation: Installation,
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
}

impl Agents {
    /// These, with the project's declaration, the text of a `project.toml` read at `at`, in place of the one beside the
    /// flows; or why it cannot be read, or lacks a file the session names, said with where it was read.
    pub fn declared(&self, text: &str, at: &str) -> Result<Agents, String> {
        let project: Project = toml::from_str(text).map_err(|e| format!("{at}: {}", e.to_string().trim_end()))?;
        for part in &self.session.parts {
            if let SessionPart::Project(ProjectPart { project: name }) = part
                && !project.files.contains_key(name)
            {
                let session = self.root.join(AGENTS_DIR).join("session.toml");
                return Err(format!("{at}: has no file {name}, which {} names", session.display()));
            }
        }
        Ok(Agents {
            project,
            ..self.clone()
        })
    }

    /// The path of a file of these, from the directory that holds the flows, with that directory's name before it, as
    /// a part names the file it comes from.
    pub fn source(&self, path: &str) -> String {
        let root = self.root.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        format!("{root}/{path}")
    }
}

/// `project.toml`: what is the project's: its design tree, its files the engine gives every agent, its settings the words
/// sent to the agents may name, its tests and its workflows.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The directory of the project's checkout that holds its design tree, from the root of the checkout.
    pub tree: String,
    /// The project's files, by the name a part of a session gives them.
    pub files: BTreeMap<String, ProjectFile>,
    /// The project's settings, which the words sent to the agents name as `project.<name>`.
    #[serde(default)]
    pub project: BTreeMap<String, String>,
    /// Where the project's tests are, and what its tests expect.
    pub tests: Option<Tests>,
    /// The project's two workflows, which the engine runs tests and checks through.
    pub ci: Option<Ci>,
    /// The paths the project protects; none where it protects none.
    #[serde(default)]
    pub protected: Protected,
}

/// The paths of the project's checkout that govern its agents, which the owner changes (principles
/// `protected-files-are-the-owners`, `named-paths-wait-for-the-owner`): where a work's own change touches one, the
/// check before the merge holds it until the owner lets it merge, asking in the words of [`Asks`].
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    /// Each path from the root of the checkout: a file, or a directory, ending in `/`, and everything in it.
    pub paths: Vec<String>,
}

/// `[tests]` of `project.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    /// Where the project's tests are, as git pathspecs.
    pub paths: Vec<String>,
    /// The expected-value data of its tests, as git pathspecs.
    pub expected: Vec<String>,
}

/// `[ci]` of `project.toml`: the project's two workflows, each with a role the engine relies on. Both report each test
/// as JUnit XML, named by the one name an agent asks for it by.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ci {
    pub fast: Option<CiFast>,
    pub full: Option<CiFull>,
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
    /// The checks it runs when named, each the name of a job of the workflow, such as the lint.
    #[serde(default)]
    pub checks: Vec<String>,
    /// The artifact a run uploads its JUnit files in.
    #[serde(default = "results")]
    pub results: String,
}

/// The full workflows: every job required to merge, dispatched on the exact commit, run once before the merge.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiFull {
    /// The workflows, by their files' names, always run by their files on the main branch.
    pub workflows: Vec<String>,
    /// Where they run, as for the fast workflow.
    pub runner: String,
    /// How long a run may take before it is given up, such as `2h`, `90m` or `30s`.
    pub too_long: String,
    /// The input that takes the commit to check out and test.
    #[serde(default = "commit_input")]
    pub commit_input: String,
    /// The input that takes a value only one request knows, which the run's name must hold.
    #[serde(default = "call_input")]
    pub call_input: String,
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

fn tests_input() -> String {
    "tests".to_string()
}

fn checks_input() -> String {
    "checks".to_string()
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

/// A file of the project, by its name in `project.toml`.
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
/// file says whether it only reads, and as the installation lets it update a lock file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YourLine {
    /// Of a line that only reads.
    pub reads_only: String,
    /// Of a line that writes.
    pub writes: String,
    /// Of a line that writes and may update a lock file, after `writes`: `{commands}` the commands it may update one
    /// with, each between backticks, joined by commas, and the last by `or`.
    pub locks: String,
    pub or: String,
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
/// again under its name where that change is another, or was answered with another option than `merges`, withdrawn
/// once the work touches none. It waits, and recommends none of its options. `ask` is what it asks, `{files}` the files
/// the change touches, each as `file` says with `{path}` its path and `{change}` how it changes (`added`, `changed` or
/// `deleted`), one to a line, and `{diff}` the change itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskProtected {
    pub name: String,
    pub ask: String,
    pub file: String,
    pub added: String,
    pub changed: String,
    pub deleted: String,
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

/// The form: the order of the fields of each kind of return, the same for every cell of a flow. The blocks a cell
/// writes go in the field [`OUTPUTS`], under their names, which the form does not list.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// What a return that asks for calls holds, in order.
    pub calls: Vec<InForm>,
    /// What a return that does the cell's work holds, in order.
    pub outcome: Vec<InForm>,
}

/// What stands at a place of the form: a group the form makes of what the flow has, or a field of the engine's by its
/// name.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum InForm {
    Group(Group),
    Field(String),
}

/// A group the form makes of what the flow has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    /// Each tool the flow names.
    Tools,
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
    pub every_time: String,
    pub fields: String,
    pub owner_reads: String,
    /// The fields the owner reads, in the order `owner_reads` names them.
    pub reads: Vec<String>,
    pub and: String,
    pub calls: String,
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
}

/// A field of a return that is the engine's, which every agent cell has: `agents/return/fields/<name>.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The kinds of return it is a field of.
    #[serde(rename = "in")]
    pub kinds: Vec<ReturnKind>,
    /// Every return of those kinds holds it.
    #[serde(default)]
    pub required: bool,
    pub told: ToldAs,
    pub words: String,
    /// What is told after the list of the fields, of a field told `listed`.
    pub also: Option<String>,
    pub shape: toml::Value,
}

/// A kind of return.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// A return that asks the engine for calls.
    Calls,
    /// A return that does the cell's work.
    Outcome,
}

impl ReturnKind {
    /// Its name, as the files write it, which is its field in a return.
    pub fn name(self) -> &'static str {
        match self {
            ReturnKind::Calls => "calls",
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
/// `{name}` a name as the return wrote it or, of what it dealt with, the trigger's name, `{value}` a value it wrote,
/// `{values}` the values of a choice, each between backticks, `{version}` a version's number and `{why}` why what came
/// back could not be read.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentBackFaults {
    pub asks_nothing: String,
    pub not_an_output: String,
    pub written_twice: String,
    pub no_value: String,
    pub not_a_value: String,
    pub not_a_choice: String,
    pub not_handed_in: String,
    pub unreadable: String,
    pub not_run_by: String,
    pub dealt_with_twice: String,
    pub dealt_with_nothing: String,
    pub written_over: String,
    pub choice_changed: String,
    pub asks_and_works: String,
    pub not_a_trigger: String,
    pub not_a_tool: String,
    pub unchanged_not_written: String,
    pub question_taken: String,
    pub no_such_question: String,
    pub question_twice: String,
    pub not_an_option: String,
    pub not_answerable: String,
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
    /// What the tool says of its own answers, by name, `{name}` places in each filled where it is said.
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
    /// How many bytes of a result of the tool `query` a message gives; a result longer is given cut and kept whole in
    /// the records. Only `query` has its results cut. With none, a result is given whole.
    pub given_bytes: Option<usize>,
    /// The shape of a request of it.
    pub request: toml::Value,
    /// What fills each place of its words that the engine fills by how the installation runs the tool, by the place's
    /// name: the words for each way, by the way's name, which the engine chooses.
    #[serde(default)]
    pub places: BTreeMap<String, BTreeMap<String, String>>,
    /// What a round is told of a request of it that cannot be read as that shape: `{error}` what the reading said.
    pub unread: Option<String>,
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
    /// What it came to: `{passed}` of `{total}`, tests and checks, apart by `came_join`.
    pub came_tests: String,
    pub came_checks: String,
    pub came_join: String,
    /// The title of the tests, and of the checks.
    pub tests_title: String,
    pub checks_title: String,
    /// A test: `{name}`, `{became}` one of the following, `{time}` as `time` says it where the results give one.
    pub test: String,
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
    /// A check, a job of the workflow: `{name}`, `{how}` one of the following, `{link}` as `link` says it.
    pub job: String,
    pub link: String,
    pub job_passed: String,
    pub job_failed: String,
    pub job_broken: String,
    pub job_pending: String,
    /// A check named that the run has no job of.
    pub job_missing: String,
    /// The run uploaded no JUnit it can read, so no test's result is known: `{why}`.
    pub no_junit: String,
    /// What the check before a merge was judged by: `{urls}` the runs.
    pub verdict_by: String,
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
    /// A merge is under way in the worktree: `{changes}`.
    pub merge_under_way: String,
    /// The worktree has changes not committed: `{changes}`.
    pub uncommitted: String,
    /// The branch has no commit of its own beyond main: `{main}`.
    pub no_own_commit: String,
    /// Tests or checks are named and the project declares no fast workflow.
    pub no_fast: String,
    /// Nothing is named and the project declares no full workflows.
    pub no_full: String,
    /// Checks named that the fast workflow does not run by name: `{unknown}`, `{runs}` what it runs.
    pub unknown_checks: String,
    /// The workflow's runner is not one this installation has: `{runner}`, `{have}`.
    pub runner_missing: String,
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
}

/// How much of a tool's result a round is given: the result is kept whole, and given cut where it is longer.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultGiven {
    /// The most bytes of a result a round is given.
    pub bound: usize,
    /// What follows a result cut: `{given}` the bytes of it given, `{size}` the bytes it is whole.
    pub cut: String,
}

/// `agents/message/message.toml`: what the engine sends a cell's agent as it enters the cell, its parts in order.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub parts: Vec<MessagePart>,
    /// What a session compacted during a round is told before the round's message is given again, whole.
    pub compacted: String,
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
    /// Of the triggers: what a trigger's line says of what happened, `{detail}` the trigger's own place.
    pub detail: Option<String>,
    /// Of the triggers: what a trigger's line says the cell does, `{do}` its words.
    #[serde(rename = "do")]
    pub does: Option<String>,
    /// What follows the list.
    pub after: Option<String>,
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
    /// Each trigger new to the cell.
    Triggers,
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
            Part::Triggers => "triggers",
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
    /// What stands in place of a datum's content where the session holds its latest version already.
    pub held: String,
    pub latest: GivenKind,
    pub last_and_latest: GivenKind,
    pub all: GivenKind,
}

/// How a datum given in one kind is put in.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GivenKind {
    pub text: String,
    /// What is sent in place of `text` where the session lacks what it needs, or `text` would be longer.
    pub whole: Option<String>,
}

/// `agents/message/results.toml`: what a round is told of the results of the calls it is given. In a text, `{name}` is
/// a tool's name, `{input}` what it was run on, `{question}` a question as it was put, `{call}` a call's number.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Results {
    pub heading: String,
    /// A tool run on an input.
    pub tool: String,
    /// A tool run on nothing.
    pub tool_alone: String,
    pub question: String,
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
}

/// What the engine's acts write into the blocks their cells write: `agents/acts/<act>.toml`.
#[derive(Clone, Debug)]
pub struct Acts {
    pub start: Start,
    pub check: Check,
    pub merge: Merge,
    pub deliver: Deliver,
}

/// `agents/acts/start.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub text: String,
}

/// `agents/acts/merge.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub text: String,
    pub moved: String,
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
    pub took_in: String,
    pub notes: String,
    pub note: String,
    pub values: CheckValues,
    pub found: Found,
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
    pub merging: String,
    pub uncommitted: String,
    pub conflict: String,
    /// The work's own change touches paths the project protects, and the owner has not let it merge.
    pub protected: String,
    /// The change under the paths the project protects could not be looked at.
    pub protected_unread: String,
    pub unnamed: String,
    pub not_head: String,
    /// The commit checked changes nothing against main: there is nothing to merge.
    pub empty: String,
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
/// file cannot be read, and why. A file the session names that `project.toml` does not have, and a name in the form
/// or in `how_you_return` that is no field of `return/fields/`, are refused as well, on the file that names them.
pub fn read(root: &Path) -> Result<Agents, String> {
    let at = root.join(AGENTS_DIR);
    let session_file = at.join("session.toml");
    let session: Session = file(&session_file)?;
    let project: Project = file(&root.join(PROJECT_FILE))?;
    let mut words = BTreeMap::new();
    for part in &session.parts {
        match part {
            SessionPart::Words(WordsPart { words: name }) => {
                words.insert(name.clone(), text(&at.join(name))?);
            }
            SessionPart::Project(ProjectPart { project: name }) if !project.files.contains_key(name) => {
                let named = format!("names the file {name}, which {PROJECT_FILE} does not have");
                return Err(format!("{}: {named}", session_file.display()));
            }
            _ => {}
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
    let in_form = (returns.form.calls.iter()).chain(&returns.form.outcome);
    let named = (in_form.filter_map(|f| match f {
        InForm::Field(name) => Some(name),
        InForm::Group(_) => None,
    }))
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
    let writes = writes(root)?;
    Ok(Agents {
        root: root.to_path_buf(),
        project,
        session,
        words,
        your_line: file(&at.join(YOUR_LINE_FILE))?,
        installation: file(&at.join(INSTALLATION_FILE))?,
        rules,
        returns,
        sent_back: file(&at.join("return").join("sent_back.toml"))?,
        fields,
        tools,
        message: file(&at.join("message").join("message.toml"))?,
        given: file(&at.join("message").join("given.toml"))?,
        results: file(&at.join("message").join("results.toml"))?,
        acts: writes.acts,
        commits: writes.commits,
        asks: asks(&at.join(ASKS_FILE))?,
    })
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
        commits: file(&at.join(COMMITS_FILE))?,
    })
}

/// The acts' words alone, read from the directory that holds the flows, as [`read`] reads them.
pub fn acts(root: &Path) -> Result<Acts, String> {
    let at = root.join(AGENTS_DIR).join(ACTS_DIR);
    Ok(Acts {
        start: file(&at.join("start.toml"))?,
        check: file(&at.join("check.toml"))?,
        merge: file(&at.join("merge.toml"))?,
        deliver: file(&at.join("deliver.toml"))?,
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
