//! The installation a try of a case runs on, apart from the real one (design document `testing`, 13): a directory of
//! its own, holding the data directory its engine keeps its store, the works' worktrees and what each round was told
//! in, and the made project's repository of its own, its main at the commit the case's item starts from. Its settings
//! are the real installation's, as the owner keeps them, but for these: its one project is the made one, declared as
//! the real installation's project is, so that the agents are sent the words a real work is sent, its repository the
//! made project's; one round at a time, since one work runs in it; no session kept warm, since the case ends with its
//! cell's run; and no copies of the store. Every act and tool of a case is the case's own, so that nothing reaches the
//! project's host.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use carry::settings::Project;

use super::made;

/// The reference of the project's own repository that holds main's head, which a run that takes main in takes in.
pub const MAIN_REF: &str = "refs/heads/main";

/// An installation a try of a case runs on.
pub struct Installation {
    /// Its data directory, an absolute path.
    pub data: PathBuf,
    /// The made project's repository of its own.
    pub repo: PathBuf,
    /// Its settings.
    pub settings: carry::Settings,
    /// The made project's name, as its settings name it.
    pub name: String,
    /// The project as the carriers carry its works out, and what is written beside the flows with its declaration.
    pub project: Project,
    pub beside: flow::agents::Agents,
}

/// The real installation's project whose settings the cases run as: the one named, or the only one there is.
pub fn real_project<'s>(settings: &'s carry::Settings, named: Option<&str>) -> Result<&'s Project, String> {
    match named {
        Some(name) => (settings.projects.get(name)).ok_or_else(|| format!("the settings name no project {name}")),
        None => match settings.projects.values().collect::<Vec<_>>().as_slice() {
            [one] => Ok(one),
            _ => Err("the settings name several projects: name the one whose settings the cases run as".into()),
        },
    }
}

/// Makes the installation in `dir`, which does not exist yet: its data directory, the made project's repository from
/// the imported one with its main at `main`, and its settings from the real installation's and its project's; with
/// `program`, the agents' command line is that program in place of the real one.
pub fn make(
    real: &carry::Settings,
    project: &Project,
    name: &str,
    beside: &flow::agents::Agents,
    imported: &Path,
    dir: &Path,
    main: &str,
    program: Option<&Path>,
) -> Result<Installation, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let data = dir.join("data");
    std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
    let repo = made::project(imported, &dir.join("project"), main)?;
    let mut settings = real.clone();
    let mut made = project.clone();
    made.repo = repo.clone();
    made.main = made::MAIN.to_string();
    settings.projects = BTreeMap::from([(name.to_string(), made)]);
    settings.agents.slots = 1;
    settings.sessions.cache_minutes = 0;
    settings.backup = None;
    if let Some(program) = program {
        settings.agents.command = program.display().to_string();
    }
    let (mut declared, faults) = carry::projects::declared(&settings, beside);
    if !faults.is_empty() {
        return Err(faults.join("; "));
    }
    let declared = declared
        .remove(name)
        .ok_or_else(|| format!("the made project {name} has no declaration"))?;
    let (project, beside) = carry::carried(declared, &beside.commits);
    Ok(Installation {
        data,
        repo,
        settings,
        name: name.to_string(),
        project,
        beside,
    })
}
