//! The system prompt of a line's session, as the engine gives it: made from the files beside the flows by the builder
//! the owner's page shows it by too ([`flow::system`]), in the parts and the order `agents/session.toml` names. It is
//! the same for every cell of the line and every session of it, so that it holds only what the line shares (principle
//! the-system-prompt-holds-what-a-line-shares).
//!
//! The prompt is made as a session begins, the project's files read from the work's worktree, where the agent reads
//! the project, and kept with the session in the data directory: every later round of the session is given the kept
//! one ([`of_session`]), so that a change of the files by the work's own commits or by main taken in reaches a session
//! only when it is reopened, as a new session. A file of the project is never read through a symbolic link
//! ([`read_within`]). The engine fills three places of the texts ([`Filling`]): `{your_line}`, what the line may do in
//! the worktree ([`Permissions::told`](crate::permissions::Permissions::told)); `{store}`, what the engine's records
//! hold, as [`crate::store`] gives it; and `{named_checks}`, the checks an agent may name to
//! `run_tests`, in the words its file gives for how the installation runs the project's CI. Each setting the texts name, but in a file of the project, is given with
//! its value beside it: the project's from its `project.toml`, and the installation's that `agents/installation.toml`
//! names, from these settings ([`named`]); so is each setting a cell's task names, as its message sends it.
//!
//! As the engine starts, every text sent to the agents is checked, each fault said with the file or the thing of the
//! flow that holds the text, all of them at once ([`faults`]): a place the engine does not fill, a setting named that
//! has no value, a set of rules or a tool that nothing beside the flows defines. So are the project's files on main,
//! where works branch from ([`project_files`]).

use std::path::{Component, Path, PathBuf};

use flow::agents::{Agents, ProjectPart, SessionPart};
use flow::{Compiled, Kind};
use uuid::Uuid;

use crate::git;
use crate::settings::{Project, Settings};

/// The directory in the data directory where the system prompt each session began with is kept.
pub const SESSIONS: &str = "sessions";

/// Where the system prompt the session began with is kept.
pub(crate) fn kept_at(data: &Path, session: Uuid) -> PathBuf {
    data.join(SESSIONS).join(format!("{session}.system.md"))
}

/// The system prompt of a round's session, kept at `at`: the kept one where the session has one, however the round
/// begins or goes on in it, so that a change of the files it is made of, by the work's own commits or by main taken
/// in, never reaches a session under way; made by `make` and kept where it has none. A session begun again under its
/// own id, as one that was cut off before it began or that the program lost, keeps the prompt it began with.
pub(crate) fn of_session(at: &Path, make: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let named = |e: std::io::Error| format!("{}: {e}", at.display());
    match std::fs::read_to_string(at) {
        Ok(kept) => return Ok(kept),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(named(e)),
    }
    let made = make()?;
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir).map_err(named)?;
    }
    // Written whole or not at all, so that a stop in the middle leaves no prompt cut short.
    let writing = at.with_extension("md.part");
    std::fs::write(&writing, &made).map_err(named)?;
    std::fs::rename(&writing, at).map_err(named)?;
    Ok(made)
}

/// The text of the file at `path` within `root`: refused where the path is not within it, or where the file or a
/// directory above it within `root` is a symbolic link, which could carry into a system prompt a file the project's
/// protection of its files does not cover; as the acts refuse a link where a worktree is ([`git::a_link`]).
pub(crate) fn read_within(root: &Path, path: &str) -> std::io::Result<String> {
    let refused = |why: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, why);
    let within = Path::new(path);
    if path.is_empty() || !within.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(refused(format!("{path:?} is no path within the repository")));
    }
    let mut at = root.to_path_buf();
    for component in within.components() {
        at.push(component);
        if git::a_link(&at) {
            let link = at.strip_prefix(root).unwrap_or(&at).display().to_string();
            return Err(refused(format!(
                "{link} is a symbolic link, which no file of the project is read through"
            )));
        }
    }
    std::fs::read_to_string(root.join(within))
}

/// What the engine puts into the texts sent to the agents of every line of the installation alike.
pub struct Filling {
    /// The settings the texts may name, with their values.
    pub named: flow::system::Settings,
    /// What fills `{named_checks}`; none where no tool's file gives words for how the installation runs its CI.
    pub named_checks: Option<String>,
    /// What fills `{store}`: what the schema's comments say of the store where the settings open it to the agents, and
    /// the tool's file's words that it is not open where they do not ([`crate::store`]).
    pub store: String,
}

/// The place of a tool's words that says which checks an agent may name.
const NAMED_CHECKS: &str = "named_checks";

/// What the engine puts into the texts, as these settings and the files beside the flows say. `{named_checks}` is
/// filled with the words its tool's file gives for how the installation runs the project's CI, as the host takes a
/// check named: by the project's fast workflow alone (`ci.fast` of its declaration), and only one it runs by name
/// (`checks`), each between backticks for `{checks}` in `named`, or `none_named` where it runs none by name; where the
/// project declares no fast workflow, nothing is run alone, `whole`.
pub fn filling(settings: &Settings, beside: &Agents) -> Filling {
    let ways = beside.tools.values().find_map(|tool| tool.places.get(NAMED_CHECKS));
    let fast = beside.project.ci.as_ref().and_then(|ci| ci.fast.as_ref());
    let named_checks = ways.and_then(|ways| match fast {
        Some(fast) if !fast.checks.is_empty() => {
            let checks: Vec<String> = fast.checks.iter().map(|c| format!("`{c}`")).collect();
            (ways.get("named")).map(|words| words.replace("{checks}", &checks.join(", ")))
        }
        Some(_) => ways.get("none_named").cloned(),
        None => ways.get("whole").cloned(),
    });
    // Settings that cannot open the store are refused as the carriers are made, before any prompt is.
    let store = crate::store(settings, beside).unwrap_or_default();
    Filling {
        named: named(settings, beside),
        named_checks,
        store,
    }
}

/// What fills each place of the texts the engine fills, by its name, for a line whose agents are told `your_line` of
/// what they may do in the worktree: `your_line`, that; `store`, what the engine's records hold, as `filling` has it;
/// `named_checks`, as `filling` has it. None for any other.
fn filled(name: &str, your_line: &str, filling: &Filling) -> Option<String> {
    match name {
        "your_line" => Some(your_line.to_string()),
        "store" => Some(filling.store.clone()),
        NAMED_CHECKS => filling.named_checks.clone(),
        _ => None,
    }
}

/// The settings the texts beside the flows may name, the installation's each with the value these settings give it.
pub fn named(settings: &Settings, beside: &Agents) -> flow::system::Settings {
    flow::system::Settings::of(&beside.project, &beside.installation, |key| settings.value(key))
}

/// The system prompt of the session of the cell's line, for a line whose agents are told `your_line` of what they may
/// do in the worktree, its project's files read by `read` by their paths within the repository; or why it cannot be
/// made.
pub(crate) fn of_line(
    beside: &Agents,
    filling: &Filling,
    flow: &Compiled,
    cell: &flow::CellSpec,
    your_line: &str,
    read: impl Fn(&str) -> std::io::Result<String>,
) -> Result<String, String> {
    let read =
        |path: &str| (read(path).map(Some)).map_err(|e| format!("{path} cannot be read in the work's worktree: {e}"));
    let parts = flow::system::parts(beside, flow, cell, read).map_err(|wrong| wrong.join("; "))?;
    let fill = |name: &str| filled(name, your_line, filling);
    flow::system::prompt(&parts, &filling.named, fill).map_err(|wrong| wrong.join("; "))
}

/// The cell's task as its message sends it, each setting it names with its value beside it; or the settings it names
/// that have no value.
pub(crate) fn task(named: &flow::system::Settings, cell: &flow::CellSpec) -> Result<Option<String>, String> {
    let Some(task) = &cell.task else {
        return Ok(None);
    };
    let lacking = |names: Vec<String>| {
        format!(
            "the task of the cell {} names settings that have no value: {}",
            cell.id.as_str(),
            names.join(", ")
        )
    };
    named.named_in(task).map(Some).map_err(lacking)
}

/// What is wrong with the texts the flow's agents are sent that is known before any session begins, every fault at
/// once, each once, with the flow and the file or the thing of the flow that holds the text: of each line's system
/// prompt, a set of rules or a tool that nothing beside the flows defines, a place the engine does not fill (a file of
/// the project standing as empty under its heading), and a setting named that has no value; and a setting named with no
/// value in a cell's task, in what a trigger says the cell does for it, or in the words of a block.
pub fn faults(settings: &Settings, beside: &Agents, flow: &Compiled) -> Vec<String> {
    let filling = filling(settings, beside);
    let named = &filling.named;
    let mut faults: Vec<String> = Vec::new();
    let mut add = |what: String| {
        let fault = format!("flow {}: {what}", flow.name);
        if !faults.contains(&fault) {
            faults.push(fault);
        }
    };
    // The prompt is the same for every cell of a line: its first cell stands for it.
    let mut lines: Vec<&str> = Vec::new();
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        let Some(line) = cell.line.as_deref() else { continue };
        if lines.contains(&line) {
            continue;
        }
        lines.push(line);
        let (parts, wrong) = flow::system::made(beside, flow, cell, |_| Ok(Some(String::new())));
        let fills = |name: &str| filled(name, "", &filling).is_some();
        for what in wrong.into_iter().chain(flow::system::check(&parts, named, fills)) {
            add(format!("the system prompt of the line {line}: {what}"));
        }
    }
    for said in flow::system::flow_texts(flow) {
        for name in named.named_in(&said.text).err().unwrap_or_default() {
            add(format!(
                "{}: names the setting `{name}`, which has no value",
                said.source
            ));
        }
    }
    faults
}

/// What is wrong with the project's files the system prompts hold, each a file a part of `agents/session.toml` names,
/// in its order, on main where works branch from, as the host last gave it: one not named by a path within the
/// repository, one main does not hold as a file, or one a symbolic link on main leads to, the file or a directory above
/// it. A work's worktree is made from main, so its sessions would find none, or read through the link.
pub fn project_files(project: &Project, beside: &Agents) -> Vec<String> {
    let repo = &project.repo;
    let main = crate::host::kept_main(project);
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
    };
    match git(&["rev-parse", "--verify", "--quiet", &format!("{main}^{{commit}}")]) {
        Ok(found) if found.status.success() => {}
        Ok(_) => {
            return vec![format!(
                "{main}, main's head where works branch from, is not in the project's repository: the files \
                 project.toml names are not looked for there"
            )];
        }
        Err(e) => return vec![format!("git could not be run to look for main's head, {main}: {e}")],
    }
    let mut wrong = Vec::new();
    for part in &beside.session.parts {
        let SessionPart::Project(ProjectPart { project: name }) = part else {
            continue;
        };
        let Some(file) = beside.project.files.get(name) else {
            continue;
        };
        let path = &file.path;
        let within = (Path::new(path).components()).all(|c| matches!(c, Component::Normal(_)));
        if path.is_empty() || !within {
            wrong.push(format!(
                "project.toml: the file {name} is named by its path within the repository, not {path:?}"
            ));
            continue;
        }
        // Each step of the path, the file last, as main's tree holds it: its mode, or nothing where it holds none.
        let mut step = PathBuf::new();
        for component in Path::new(path).components() {
            step.push(component);
            let step = step.display().to_string();
            let listed = match git(&["ls-tree", &main, "--", &step]) {
                Ok(listed) if listed.status.success() => String::from_utf8_lossy(&listed.stdout).into_owned(),
                Ok(listed) => {
                    let said = String::from_utf8_lossy(&listed.stderr).trim().to_string();
                    wrong.push(format!("project.toml: git could not look for {path} on {main}: {said}"));
                    break;
                }
                Err(e) => {
                    wrong.push(format!("project.toml: git could not be run to look for {path}: {e}"));
                    break;
                }
            };
            let mode = listed.split_whitespace().next().unwrap_or_default().to_string();
            let last = step == *path;
            let fault = match mode.as_str() {
                "" => format!("the file {name}, {path}, is not on main, {main}, where works branch from"),
                "120000" => format!(
                    "the file {name}, {path}: {step} is a symbolic link on main, {main}, which no file of the project \
                     is read through"
                ),
                "100644" | "100755" if last => continue,
                "040000" if !last => continue,
                _ => format!("the file {name}, {path}, is not a file on main, {main}"),
            };
            wrong.push(format!("project.toml: {fault}"));
            break;
        }
    }
    wrong
}

#[cfg(test)]
mod tests;
