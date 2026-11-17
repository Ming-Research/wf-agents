//! `wf-agents cases`: the cell cases of a flow (design document `testing`, 10 to 17), kept beside the flows in
//! `cases/<flow>/`, the made project they run in in `cases/projects/`.
//!
//! - [`made`]: the made project's repositories.

pub mod made;

use std::path::{Path, PathBuf};

use flow::Compiled;

/// A flow's cases read and checked against its definition, and where its made project is kept.
pub struct Flow {
    pub cases: ::cases::Cases,
    /// The made project's export.
    pub export: PathBuf,
}

/// The directory of a flow's cases, beside the flows: `cases/<flow>/` of the directory that holds the flow's.
pub fn dir_of(flow_dir: &Path, compiled: &Compiled) -> Option<PathBuf> {
    Some(flow_dir.parent()?.join("cases").join(&compiled.name))
}

/// Compiles the flow in the directory, reads its cases from `cases` or from beside the flows, and checks them; every
/// fault is said.
pub fn read(flow_dir: &Path, cases_dir: Option<&Path>, has: &flow::Has) -> Result<Flow, Vec<String>> {
    let shown = |e: &dyn std::fmt::Display| vec![format!("{}: {e}", flow_dir.display())];
    let source = flow::read(flow_dir).map_err(|e| shown(&e))?;
    let compiled = flow::compile(&source, has).map_err(|faults| {
        let n = faults.len();
        let said = format!("does not compile ({n} faults): `wf-agents flow check` says each");
        shown(&said)
    })?;
    let dir = match cases_dir {
        Some(dir) => dir.to_path_buf(),
        None => dir_of(flow_dir, &compiled).ok_or_else(|| shown(&"the flows' directory holds it, and it has none"))?,
    };
    let in_dir = |faults: Vec<::cases::Fault>| {
        faults
            .iter()
            .map(|f| format!("{}/{f}", dir.display()))
            .collect::<Vec<String>>()
    };
    let read = ::cases::read(&dir).map_err(in_dir)?;
    let checked = ::cases::check(&compiled, &read).map_err(in_dir)?;
    let projects = dir
        .parent()
        .map_or_else(|| dir.join(".."), Path::to_path_buf)
        .join(made::PROJECTS);
    let export = projects.join(format!("{}.fast-export", checked.project));
    Ok(Flow { cases: checked, export })
}

/// Every name of the made project the cases use: the commit each item starts from, each a round commits and each main
/// moves to, each with where it is used.
pub fn commits(cases: &::cases::Cases) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (n, item) in &cases.items {
        out.push((item.main.clone(), format!("item {n}")));
    }
    for of in &cases.cells {
        for case in &of.cases {
            for step in &case.history {
                match step {
                    ::cases::Step::Round(round) => out.extend(round.commit.clone().map(|c| (c, case.id.clone()))),
                    ::cases::Step::Main(main) => out.push((main.clone(), case.id.clone())),
                    _ => {}
                }
            }
        }
    }
    out
}

/// `wf-agents cases check`: the flow's cases read and checked, every name of the made project they use found in it.
/// A line for each cell, or every fault.
pub fn check(flow_dir: &Path, cases_dir: Option<&Path>, has: &flow::Has) -> Result<Vec<String>, Vec<String>> {
    let flow = read(flow_dir, cases_dir, has)?;
    let dir = tempfile::tempdir().map_err(|e| vec![format!("a directory to import the made project into: {e}")])?;
    let imported = made::import(&flow.export, &dir.path().join("made.git")).map_err(|e| vec![e])?;
    let mut missing: Vec<String> = (commits(&flow.cases).into_iter())
        .filter_map(|(name, by)| made::commit(&imported, &name).err().map(|e| format!("{by}: {e}")))
        .collect();
    // Each item's work starts from a repository of the project's own, its main at the item's commit.
    let all_found = missing.is_empty();
    for (n, item) in flow.cases.items.iter().filter(|_| all_found) {
        let at = dir.path().join(format!("item-{n}"));
        match made::project(&imported, &at, &item.main) {
            Ok(repo) => missing.extend(made::remove(&repo).err()),
            Err(why) => missing.push(format!("item {n}: {why}")),
        }
    }
    if !missing.is_empty() {
        return Err(missing);
    }
    Ok((flow.cases.cells.iter())
        .map(|of| format!("{}: {} cases, in {}", of.cell, of.cases.len(), of.file))
        .collect())
}
