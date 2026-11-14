//! `wf-agents flow situations`: what each agent's cell of a definition is sent in every situation an entry of it can
//! reach (`context::situations`), a file for each situation in a directory for the cell. A file holds the situation,
//! as its keys write it; the system prompt of the cell's line, made as the engine makes it (`flow::system`); the
//! message, built by the builder the engine sends it with (`context::message`); and the form of a return, made as the
//! engine gives it to the agents' program (`flow::form`). What a run fills is left open as `{name}`, and a setting a
//! text names is given beside its name where a run gives it, its value left open as `{name}` too: the command reads
//! nothing of an installation or a work. The project's files the system prompt holds are read from a checkout where one
//! is given, and are left out where none is.

use std::fmt::Write as _;
use std::path::Path;

use context::Values;
use context::situations::{self, Named};
use flow::agents::Agents;
use flow::{CellSpec, Compiled, Kind};

/// What the command is given.
pub struct Asked<'a> {
    /// The definition's directory.
    pub dir: &'a Path,
    /// Where each cell's directory is made.
    pub out: &'a Path,
    /// The one cell written, by its id; every agent's cell where none is.
    pub cell: Option<&'a str>,
    /// The project's checkout, whose files the system prompt holds.
    pub project: Option<&'a Path>,
}

/// Writes each agent's cell's situations, as the module says, the definition compiled against what the engine has;
/// for each cell, a line of how many it wrote, where, and how many combinations of each datum's own state there are.
pub fn write(asked: &Asked, has: &flow::Has) -> Result<Vec<String>, String> {
    let shown = |e: &dyn std::fmt::Display| format!("{}: {e}", asked.dir.display());
    let source = flow::read(asked.dir).map_err(|e| shown(&e))?;
    let compiled = flow::compile(&source, has).map_err(|faults| {
        let n = faults.len();
        format!(
            "{} does not compile ({n} faults): `wf-agents flow check` says each",
            asked.dir.display()
        )
    })?;
    let root = (asked.dir.parent()).ok_or_else(|| shown(&"the flows' directory holds it, and it has none"))?;
    let beside = flow::agents::read(root)?;
    let form = flow::form::form(&compiled, &beside)?;
    let cells: Vec<&CellSpec> = (compiled.cells.iter())
        .filter(|c| c.kind == Kind::Agent && asked.cell.is_none_or(|id| c.id.as_str() == id))
        .collect();
    if let (Some(id), true) = (asked.cell, cells.is_empty()) {
        return Err(format!("{} has no agent's cell {id}", compiled.name));
    }
    // A setting a text names is given beside its name as a run gives it, its value the place `{name}`.
    let named = flow::system::Settings::of(&beside.project, |key| Some(format!("{{{key}}}")));
    let mut said = Vec::new();
    for cell in cells {
        let at = asked.out.join(cell.id.as_str());
        let held = std::fs::read_dir(&at).map(|mut entries| entries.next().is_some());
        if held.unwrap_or(false) {
            return Err(format!("{} holds files already", at.display()));
        }
        std::fs::create_dir_all(&at).map_err(|e| format!("{}: {e}", at.display()))?;
        let system = system(&beside, &compiled, cell, asked.project, &named)?;
        let all = situations::situations(&compiled, &beside, cell, driver::sessions::SESSION);
        for (n, situation) in all.iter().enumerate() {
            let sent = context::message(&compiled, &beside, cell, &situation.situation, Values::Slots(&named));
            let message = context::message::text(&sent);
            let path = at.join(format!("{:03}-{}.md", n + 1, situation.name));
            let text = file(situation, &system, &message, &form);
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        let every = situations::combinations(&compiled, &beside, cell, driver::sessions::SESSION);
        said.push(format!(
            "{}: {} situations, in {}; {every} combinations of each datum's own state",
            cell.id.as_str(),
            all.len(),
            at.display()
        ));
    }
    Ok(said)
}

/// The system prompt of the cell's line, put together as the engine puts it together (`flow::system::prompt`): each
/// place the engine fills left as `{name}`, and each setting named given beside its name as these settings give it.
fn system(
    beside: &Agents,
    flow: &Compiled,
    cell: &CellSpec,
    project: Option<&Path>,
    named: &flow::system::Settings,
) -> Result<String, String> {
    let read = |path: &str| Ok(project.and_then(|checkout| std::fs::read_to_string(checkout.join(path)).ok()));
    let parts = flow::system::parts(beside, flow, cell, read).map_err(|wrong| wrong.join("; "))?;
    let open = |name: &str| Some(format!("{{{name}}}"));
    flow::system::prompt(&parts, named, open).map_err(|wrong| wrong.join("; "))
}

/// A situation's file: the situation, then the system prompt, the message and the form, each under a line that says
/// which it is.
fn file(named: &Named, system: &str, message: &str, form: &str) -> String {
    let s = &named.situation;
    let list = |items: &[String]| {
        let quoted: Vec<String> = items.iter().map(|i| format!("{i:?}")).collect();
        format!("[{}]", quoted.join(", "))
    };
    let mut text = String::new();
    let _ = writeln!(text, "situation = {:?}", named.name);
    let _ = writeln!(text, "holds = {}", list(&s.holds));
    let _ = writeln!(text, "after_hand_in = {}", s.after_hand_in);
    let _ = writeln!(text, "triggers = {}", list(&s.triggers));
    if let Some(main) = &s.main {
        let _ = writeln!(text, "main = {main:?}");
    }
    let _ = writeln!(text, "results = {}", list(&s.results));
    let _ = writeln!(text, "sent_back = {}", s.sent_back);
    let _ = writeln!(text, "\n[data]");
    for (block, stands) in &s.data {
        let _ = writeln!(text, "{block} = {:?}", stands.name());
    }
    for (what, body) in [("system prompt", system), ("message", message), ("return form", form)] {
        let _ = write!(text, "\n===== {what} =====\n\n{}\n", body.trim_end());
    }
    text
}
