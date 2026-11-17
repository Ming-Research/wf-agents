//! `wf-agents cases`: the cell cases of a flow (design document `testing`, 10 to 17), kept beside the flows in
//! `cases/<flow>/`, the made project they run in in `cases/projects/`.
//!
//! - [`made`]: the made project's repositories.
//! - [`installation`]: the installation a try of a case runs on, apart from the real one.
//! - [`carriers`]: what carries out the rounds, acts and calls of a case's work.
//! - [`build`](mod@build): a try of a case built through the engine, its cell's first round sent.

pub mod build;
pub mod carriers;
pub mod installation;
pub mod made;

use std::collections::BTreeMap;
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

/// What `wf-agents cases build` is asked.
pub struct Asked<'a> {
    pub flow_dir: &'a Path,
    pub cases_dir: Option<&'a Path>,
    /// The real installation's settings, and the project of it whose settings the cases run as.
    pub settings: &'a Path,
    pub project: Option<&'a str>,
    /// The cases, by their cells and ids; every case where neither is named.
    pub cells: &'a [String],
    pub ids: &'a [String],
    /// The directory the runs' installations are made in.
    pub runs: &'a Path,
}

/// Where under the data directory the runs of the cases make their installations.
pub const RUNS: &str = "cell-cases/runs";

/// The most cases built or run at once (design document `testing`, 13).
pub const AT_ONCE: usize = 10;

/// `wf-agents cases build`: each case asked built through the engine with a stand-in for the agents' program, at most
/// [`AT_ONCE`] at a time; a line for each, saying what ran its cell and the hashes of what its cell's line was sent, or
/// why it was refused. What each build made is removed.
pub fn build_cases(asked: &Asked, has: &flow::Has) -> Result<Vec<String>, Vec<String>> {
    if !asked.runs.as_os_str().is_empty() {
        return Err(vec!["the cases are not built yet".to_string()]);
    }
    let flow_dir = asked.flow_dir;
    let read = read(flow_dir, asked.cases_dir, has)?;
    let source = flow::read(flow_dir).map_err(|e| vec![format!("{}: {e}", flow_dir.display())])?;
    let compiled = flow::compile(&source, has).map_err(|_| vec![format!("{} does not compile", flow_dir.display())])?;
    let beside_dir = flow_dir
        .parent()
        .ok_or_else(|| vec!["the flows' directory".to_string()])?;
    let beside = flow::agents::read(beside_dir).map_err(|e| vec![e])?;
    let settings = carry::Settings::load(asked.settings).map_err(|e| vec![e])?;
    let project = installation::real_project(&settings, asked.project).map_err(|e| vec![e])?;
    let stand_in = std::env::current_exe()
        .map(|exe| exe.with_file_name("wf-agents-stand-in"))
        .map_err(|e| vec![format!("the stand-in for the agents' program: {e}")])?;
    std::fs::create_dir_all(asked.runs).map_err(|e| vec![format!("{}: {e}", asked.runs.display())])?;
    let run = tempfile::Builder::new()
        .prefix("build-")
        .tempdir_in(asked.runs)
        .map_err(|e| vec![format!("{}: {e}", asked.runs.display())])?;
    let imported = made::import(&read.export, &run.path().join("made.git")).map_err(|e| vec![e])?;
    let mut commits = BTreeMap::new();
    for (name, by) in self::commits(&read.cases) {
        let id = made::commit(&imported, &name).map_err(|e| vec![format!("{by}: {e}")])?;
        commits.insert(name, id);
    }
    let chosen: Vec<&::cases::Case> = (read.cases.cells.iter())
        .filter(|of| asked.cells.is_empty() || asked.cells.iter().any(|c| c == of.cell.as_str()))
        .flat_map(|of| &of.cases)
        .filter(|case| asked.ids.is_empty() || asked.ids.contains(&case.id))
        .collect();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| vec![format!("the runtime: {e}")])?;
    let (compiled, settings, beside, commits) = (&compiled, &settings, &beside, &commits);
    let (imported, stand_in, made_name) = (imported.as_path(), stand_in.as_path(), read.cases.project.as_str());
    let built = runtime.block_on(async {
        let mut said = Vec::new();
        for batch in chosen.chunks(AT_ONCE) {
            let mut tries = Vec::new();
            for case in batch.iter().copied() {
                let Some(item) = read.cases.items.get(&case.item) else {
                    said.push(Err(format!("{}: no item {}", case.id, case.item)));
                    continue;
                };
                let dir = build::try_dir(run.path(), &case.id, 1);
                tries.push(async move {
                    let with = build::With {
                        flow: compiled,
                        case,
                        item,
                        settings,
                        project,
                        made: made_name,
                        imported,
                        commits,
                        beside,
                        dir: &dir,
                        program: Some(stand_in),
                    };
                    built_line(&with, case).await
                });
            }
            said.extend(futures_all(tries).await);
        }
        said
    });
    let (lines, faults): (Vec<_>, Vec<_>) = built.into_iter().partition(Result::is_ok);
    let faults: Vec<String> = faults.into_iter().filter_map(Result::err).collect();
    match faults.is_empty() {
        true => Ok(lines.into_iter().filter_map(Result::ok).collect()),
        false => Err(faults),
    }
}

/// Runs the futures side by side on the runtime, giving their outputs in order.
async fn futures_all<F: std::future::Future>(futures: Vec<F>) -> Vec<F::Output> {
    let mut pinned: Vec<std::pin::Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();
    let mut outputs: Vec<Option<F::Output>> = pinned.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut all = true;
        for (future, output) in pinned.iter_mut().zip(outputs.iter_mut()) {
            if output.is_none() {
                match future.as_mut().poll(cx) {
                    std::task::Poll::Ready(value) => *output = Some(value),
                    std::task::Poll::Pending => all = false,
                }
            }
        }
        match all {
            true => std::task::Poll::Ready(()),
            false => std::task::Poll::Pending,
        }
    })
    .await;
    outputs.into_iter().flatten().collect()
}

/// A case built, as a line: what ran its cell, whether it ran before, and the hashes of what its line was sent; or why
/// it was refused. What it made is removed either way.
async fn built_line(with: &build::With<'_>, case: &::cases::Case) -> Result<String, String> {
    let building = build::build(with).await;
    let building = match building {
        Ok(building) => building,
        Err(why) => {
            let _ = made::remove(with.dir);
            return Err(why);
        }
    };
    let entered = building.entered_as_said(case);
    let built = building.built.clone();
    let dir = building.installation.data.clone();
    let tag = carry::git::tag(&carry::session::installation(&dir).map_err(|e| format!("{}: {e}", case.id))?);
    let known: Vec<&String> = with.commits.values().collect();
    let place = with.dir.display().to_string();
    let hash = |text: &str| {
        sha1_smol::Sha1::from(masked(text, &place, &tag, &known))
            .digest()
            .to_string()
    };
    let reading = built.reading.as_deref().map(hash).unwrap_or_else(|| "none".to_string());
    let line = format!(
        "{}: {} entered for {}, {}; system {}, reading {}, message {}",
        case.id,
        case.cell,
        built.new.join(", "),
        if built.ran_before { "having run before" } else { "first" },
        &hash(&built.system)[..12],
        reading.get(..12).unwrap_or(reading.as_str()),
        &hash(&built.message)[..12],
    );
    building.remove()?;
    entered.map(|()| line)
}

/// The text with what differs from one try of a case to the next said alike: the directory the try was made in, the
/// installation's tag in its branch's name, and every commit id the made project does not have, which a try makes as
/// it takes main in.
pub fn masked(text: &str, place: &str, tag: &str, known: &[&String]) -> String {
    let text = text.replace(place, "{installation}").replace(tag, "{tag}");
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let hex = word.len() >= 7
            && word.len() <= 40
            && word.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        let made = known.iter().any(|id| id.starts_with(word.as_str()));
        match hex && !made {
            true => out.push_str("{commit}"),
            false => out.push_str(word),
        }
        word.clear();
    };
    for c in text.chars() {
        match c.is_ascii_alphanumeric() {
            true => word.push(c),
            false => {
                flush(&mut word, &mut out);
                out.push(c);
            }
        }
    }
    flush(&mut word, &mut out);
    out
}
