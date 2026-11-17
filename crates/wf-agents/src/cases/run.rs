//! `wf-agents cases run` and `wf-agents cases show` (design document `testing`, 13 to 16). A run first builds each
//! case asked with a stand-in for the agents' program, which gives the key of its texts with no model: the hashes of the
//! system prompt of its cell's line, of the reading round's message and of its cell's first message, as the engine sent
//! them, and of the words beside the flows and the cell's own file, which hold what later rounds are sent. A case whose
//! key, model, effort and agents' program a kept try already has stands, judged again where only what it owes changed;
//! every other runs, at most ten at once, until what it spends would pass the bound it is given. A try runs the case on
//! the real agents' program, on the model and effort its cell's line runs on, and is judged mechanically, then by
//! reading where the case says what reading judges; a wrong return asks a fork of the last round's session about the
//! cell's text at once, and a case with a wrong return runs twice more. Every try is kept, in
//! `cell-cases/results/<flow>/<cell>/<case>.json` of the data directory, and only added to.
//!
//! What the run says is ready for a pull request's comment: each case, its tries before and after the text changed,
//! passed or failed, and for each that failed, what was found and what the fork said of the text.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use cases::judged::Did;
use serde::{Deserialize, Serialize};

use super::{AT_ONCE, ask, build, made, masked};

/// Where under the data directory the kept results are.
pub const RESULTS: &str = "cell-cases/results";

/// How many times a case runs where it fails (design document `testing`, 13).
pub const TRIES: usize = 3;

/// The key of a case's texts: what decides whether a kept try still stands for them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub system: String,
    pub reading: String,
    pub message: String,
    pub words: String,
    pub model: String,
    pub effort: String,
    pub program: String,
}

/// A try of a case, as it is kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Try {
    pub key: Key,
    /// The hash of the case as written, its history, situation and what it owes.
    pub case: String,
    /// The commit of the repository the flow was read from, where it is one.
    pub flow_at: String,
    pub passed: bool,
    pub mechanical: Vec<String>,
    pub reading: Option<(bool, String)>,
    pub fork: Option<String>,
    /// Of the fork's first request, the tokens read from the session's cache and those sent fresh.
    #[serde(default)]
    pub fork_cache: Option<(u64, u64)>,
    pub did: Did,
    /// Each round of the cell: its run and number, what came back, what it cost in millionths of a dollar.
    pub rounds: Vec<(u64, usize, String, u64)>,
    pub cost_usd: f64,
}

/// A case's kept tries, in order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub case: String,
    pub cell: String,
    pub tries: Vec<Try>,
}

fn kept_at(data: &Path, flow: &str, cell: &str, case: &str) -> PathBuf {
    data.join(RESULTS).join(flow).join(cell).join(format!("{case}.json"))
}

/// The kept tries of a case; none where none are kept.
pub fn kept(data: &Path, flow: &str, cell: &str, case: &str) -> Result<Kept, String> {
    let at = kept_at(data, flow, cell, case);
    match std::fs::read_to_string(&at) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", at.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Kept {
            case: case.to_string(),
            cell: cell.to_string(),
            tries: Vec::new(),
        }),
        Err(e) => Err(format!("{}: {e}", at.display())),
    }
}

fn keep(data: &Path, flow: &str, kept: &Kept) -> Result<(), String> {
    let at = kept_at(data, flow, &kept.cell, &kept.case);
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(kept).map_err(|e| e.to_string())?;
    std::fs::write(&at, text).map_err(|e| format!("{}: {e}", at.display()))
}

fn sha(text: &str) -> String {
    sha1_smol::Sha1::from(text).digest().to_string()
}

/// The hash of the words beside the flows, every file under them in the order of its path, and of the cell's own file:
/// what the rounds after a cell's first are sent, results and sending back among it, is written there.
fn words_hash(flows: &Path, flow_dir: &Path, cell: &str) -> String {
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            match path.is_dir() {
                true => files(&path, out),
                false => out.push(path),
            }
        }
    }
    let mut all = Vec::new();
    files(&flows.join("agents"), &mut all);
    all.push(flow_dir.join(flow::CELLS_DIR).join(format!("{cell}.toml")));
    all.sort();
    let mut text = String::new();
    for path in all {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = write!(
            text,
            "{}\n{}\n",
            path.strip_prefix(flows).unwrap_or(&path).display(),
            content
        );
    }
    sha(&text)
}

/// What `wf-agents cases run` is asked.
pub struct Asked<'a> {
    pub cases: super::Asked<'a>,
    /// The data directory, under which the runs are made and the results kept.
    pub data: &'a Path,
    /// The most the run may spend, in dollars.
    pub spend: f64,
    /// Run every case asked, whatever is kept.
    pub all: bool,
    /// The agents' program, where another stands in for the installation's: for a run with no model.
    pub program: Option<&'a Path>,
    /// Run what is to run; without it, only say what would, and how the kept tries stand.
    pub go: bool,
}

/// What became of a case in this run.
enum Became {
    Ran(Kept),
    Stood(Kept),
    Refused(String),
    Unspent(String),
    /// It would run, its texts having no kept try, but the run was not told to go.
    Planned(Kept),
}

/// `wf-agents cases run`: the cases asked, each built for its key, run where no kept try stands for it, judged and
/// kept; what it says, ready for a pull request's comment.
pub fn run(asked: &Asked, has: &flow::Has) -> Result<String, Vec<String>> {
    let c = &asked.cases;
    let read = super::read(c.flow_dir, c.cases_dir, has)?;
    let source = flow::read(c.flow_dir).map_err(|e| vec![format!("{}: {e}", c.flow_dir.display())])?;
    let compiled =
        flow::compile(&source, has).map_err(|_| vec![format!("{} does not compile", c.flow_dir.display())])?;
    let flows = c
        .flow_dir
        .parent()
        .ok_or_else(|| vec!["the flows' directory".to_string()])?;
    let beside = flow::agents::read(flows).map_err(|e| vec![e])?;
    let cases_root = read
        .export
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let words = ask::words(&cases_root).map_err(|e| vec![e])?;
    let settings = carry::Settings::load(c.settings).map_err(|e| vec![e])?;
    let project = super::installation::real_project(&settings, c.project).map_err(|e| vec![e])?;
    let stand_in = std::env::current_exe()
        .map(|exe| exe.with_file_name("wf-agents-stand-in"))
        .map_err(|e| vec![format!("the stand-in for the agents' program: {e}")])?;
    std::fs::create_dir_all(c.runs).map_err(|e| vec![format!("{}: {e}", c.runs.display())])?;
    let dir = tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(c.runs)
        .map_err(|e| vec![format!("{}: {e}", c.runs.display())])?;
    let imported = made::import(&read.export, &dir.path().join("made.git")).map_err(|e| vec![e])?;
    let mut commits = BTreeMap::new();
    for (name, by) in super::commits(&read.cases) {
        let id = made::commit(&imported, &name).map_err(|e| vec![format!("{by}: {e}")])?;
        commits.insert(name, id);
    }
    let flow_at = made::git(c.flow_dir, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());
    let mut real = settings.clone();
    if let Some(program) = asked.program {
        real.agents.command = program.display().to_string();
    }
    let program = ask::version(&real);
    let chosen: Vec<&cases::Case> = (read.cases.cells.iter())
        .filter(|of| c.cells.is_empty() || c.cells.iter().any(|cell| cell == of.cell.as_str()))
        .flat_map(|of| &of.cases)
        .filter(|case| c.ids.is_empty() || c.ids.contains(&case.id))
        .collect();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| vec![format!("the runtime: {e}")])?;
    let mut spent = 0.0;
    let mut became = Vec::new();
    for batch in chosen.chunks(AT_ONCE) {
        let mut tries = Vec::new();
        for case in batch.iter().copied() {
            let Some(item) = read.cases.items.get(&case.item) else {
                became.push((case, Became::Refused(format!("no item {}", case.item))));
                continue;
            };
            let spec = (compiled.cells.iter()).find(|s| s.id == case.cell);
            let ran_on = spec.map(|s| settings.ran_on(Some(&settings.items.default_level), s));
            let Some(Ok(ran_on)) = ran_on else {
                became.push((case, Became::Refused("its cell runs on no model".into())));
                continue;
            };
            let kept = match kept(asked.data, &compiled.name, case.cell.as_str(), &case.id) {
                Ok(kept) => kept,
                Err(why) => {
                    became.push((case, Became::Refused(why)));
                    continue;
                }
            };
            let estimate = estimate(&kept);
            if spent + estimate > asked.spend {
                became.push((case, Became::Unspent(format!("it would cost about ${estimate:.2}"))));
                continue;
            }
            spent += estimate;
            let one = One {
                compiled: &compiled,
                case,
                item,
                settings: &settings,
                real: &real,
                project,
                made: &read.cases.project,
                imported: &imported,
                commits: &commits,
                beside: &beside,
                rules: &read.cases.rules,
                tests: read.cases.tests.as_ref(),
                runs: dir.path(),
                stand_in: &stand_in,
                words: &words,
                flows,
                flow_dir: c.flow_dir,
                ran_on: (ran_on.model.clone(), ran_on.effort.clone().unwrap_or_default()),
                program: &program,
                flow_at: &flow_at,
                all: asked.all,
                go: asked.go,
            };
            tries.push(async move { (case, one.go(kept).await) });
        }
        for (case, result) in runtime.block_on(super::futures_all(tries)) {
            if let Became::Ran(kept) = &result
                && let Err(why) = keep(asked.data, &compiled.name, kept)
            {
                became.push((case, Became::Refused(why)));
                continue;
            }
            became.push((case, result));
        }
    }
    Ok(said(&compiled.name, &flow_at, &became))
}

/// What a case is expected to cost: its last try's cost where one is kept, else nothing known, said as nothing.
fn estimate(kept: &Kept) -> f64 {
    kept.tries.last().map_or(0.0, |t| t.cost_usd)
}

/// A case to run, with all it is run with.
struct One<'a> {
    compiled: &'a flow::Compiled,
    case: &'a cases::Case,
    item: &'a cases::written::Item,
    settings: &'a carry::Settings,
    /// The settings with the agents' program the run uses.
    real: &'a carry::Settings,
    project: &'a carry::settings::Project,
    made: &'a str,
    imported: &'a Path,
    commits: &'a BTreeMap<String, String>,
    beside: &'a flow::agents::Agents,
    rules: &'a BTreeMap<String, cases::written::Rules>,
    tests: Option<&'a cases::written::Tests>,
    runs: &'a Path,
    stand_in: &'a Path,
    words: &'a ask::Words,
    flows: &'a Path,
    flow_dir: &'a Path,
    ran_on: (String, String),
    program: &'a str,
    flow_at: &'a str,
    all: bool,
    go: bool,
}

impl One<'_> {
    fn with<'w>(
        &'w self,
        dir: &'w Path,
        program: Option<&'w Path>,
        settings: &'w carry::Settings,
        first_only: bool,
    ) -> build::With<'w> {
        build::With {
            flow: self.compiled,
            case: self.case,
            item: self.item,
            settings,
            project: self.project,
            made: self.made,
            imported: self.imported,
            commits: self.commits,
            beside: self.beside,
            rules: self.rules,
            tests: self.tests,
            dir,
            program,
            first_only,
        }
    }

    /// Builds the case for its key, and runs it where no kept try stands for it.
    async fn go(self, mut kept: Kept) -> Became {
        let case = self.case;
        let dir = build::try_dir(self.runs, &format!("{}-key", case.id), 1);
        let key = match self.key(&dir).await {
            Ok(key) => key,
            Err(why) => return Became::Refused(why),
        };
        let hash = sha(&format!("{case:?}"));
        let standing = kept.tries.iter().position(|t| t.key == key);
        if let (Some(at), false) = (standing, self.all) {
            if kept.tries[at].case != hash {
                // Only what the case owes changed: its kept tries are judged again.
                let spec = (self.compiled.cells.iter()).find(|s| s.id == case.cell);
                for t in kept.tries.iter_mut().filter(|t| t.key == key) {
                    let found = spec.map(|s| cases::judged::judge(self.compiled, s, &case.owes, &t.did));
                    t.mechanical = found.unwrap_or_default();
                    t.passed = t.mechanical.is_empty() && t.reading.as_ref().is_none_or(|(p, _)| *p);
                    t.case = hash.clone();
                }
                return Became::Ran(kept);
            }
            return Became::Stood(kept);
        }
        if !self.go {
            return Became::Planned(kept);
        }
        for n in 1..=TRIES {
            let t = self.attempt(n, &key, &hash).await;
            let passed = t.passed;
            kept.tries.push(t);
            if n == 1 && passed {
                break;
            }
        }
        Became::Ran(kept)
    }

    /// The key of the case's texts, built with the stand-in.
    async fn key(&self, dir: &Path) -> Result<Key, String> {
        let with = self.with(dir, Some(self.stand_in), self.settings, true);
        let building = match build::build(&with).await {
            Ok(building) => building,
            Err(why) => {
                let _ = made::remove(dir);
                return Err(why);
            }
        };
        let entered = building.entered_as_said(self.case);
        let id = carry::session::installation(&building.installation.data)?;
        let tag = carry::git::tag(&id);
        let known: Vec<&String> = self.commits.values().collect();
        let place = dir.display().to_string();
        let hash = |text: &str| sha(&masked(text, &place, &tag, &known));
        let built = &building.built;
        let key = Key {
            system: hash(&built.system),
            reading: built.reading.as_deref().map(hash).unwrap_or_default(),
            message: hash(&built.message),
            words: words_hash(self.flows, self.flow_dir, self.case.cell.as_str()),
            model: self.ran_on.0.clone(),
            effort: self.ran_on.1.clone(),
            program: self.program.to_string(),
        };
        building.remove()?;
        entered.map(|()| key)
    }

    /// A try of the case on the agents' program, judged and, where its return was wrong, the fork asked.
    async fn attempt(&self, n: usize, key: &Key, hash: &str) -> Try {
        let case = self.case;
        let dir = build::try_dir(self.runs, &case.id, n);
        let mut t = Try {
            key: key.clone(),
            case: hash.to_string(),
            flow_at: self.flow_at.to_string(),
            ..Try::default()
        };
        let program = (self.real.agents.command != self.settings.agents.command).then_some(self.stand_in);
        let with = self.with(&dir, program, self.real, false);
        let building = match build::build(&with).await {
            Ok(building) => building,
            Err(why) => {
                let _ = made::remove(&dir);
                t.mechanical = vec![why];
                return t;
            }
        };
        let spec = (self.compiled.cells.iter()).find(|s| s.id == case.cell);
        let did = building.did(self.compiled, case).unwrap_or_default();
        let mut found = spec
            .map(|s| cases::judged::judge(self.compiled, s, &case.owes, &did))
            .unwrap_or_default();
        if let Some(tool) = &building.unanswered {
            found.push(format!("called {tool}, which the case holds no result for"));
        }
        let rounds = building.rounds(self.compiled, case).unwrap_or_default();
        t.cost_usd = rounds.iter().map(|r| r.usd_micros as f64 / 1e6).sum();
        t.rounds = (rounds.iter())
            .map(|r| (r.run.0, r.number, r.back.clone(), r.usd_micros))
            .collect();
        let last = rounds.last();
        if found.is_empty()
            && let (Some(judge), Some(first), Some(last)) = (&case.judge, rounds.first(), last)
        {
            let asked = (judge.as_str(), first.message.as_str(), last.back.as_str());
            t.reading = Some(match ask::judge(self.real, self.words, &dir, &self.ran_on.0, asked) {
                Ok((passed, why, cost)) => {
                    t.cost_usd += cost;
                    (passed, why)
                }
                Err(why) => (false, format!("the reading judge could not judge: {why}")),
            });
        }
        t.passed = found.is_empty() && t.reading.as_ref().is_none_or(|(p, _)| *p);
        t.mechanical = found;
        if !t.passed
            && let Some(last) = last
        {
            let words = self.words;
            match building
                .fork(self.compiled, last.run, last.number, &words.fork, &words.fork_refused)
                .await
            {
                Ok(forked) => {
                    t.cost_usd += forked.cost_usd;
                    let observations = (forked.answer.as_ref())
                        .and_then(|a| a["outcome"]["observations"].as_array().cloned())
                        .map(|o| o.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" "));
                    t.fork = Some(observations.filter(|o| !o.is_empty()).unwrap_or(forked.text));
                    t.fork_cache = Some((forked.cached, forked.fresh));
                }
                Err(why) => t.fork = Some(format!("The fork could not be asked: {why}")),
            }
        }
        t.did = did;
        let _ = building.remove();
        t
    }
}

/// How a case's tries stand, in words: how many of them passed.
fn verdict(tries: &[&Try]) -> String {
    match tries.len() {
        0 => "—".to_string(),
        n => {
            let passed = tries.iter().filter(|t| t.passed).count();
            match passed == n {
                true => format!("passed ({passed}/{n})"),
                false => format!("failed ({passed}/{n} passed)"),
            }
        }
    }
}

/// What the run says, ready for a pull request's comment: a table of the cases, before and after, and what was found
/// of each that failed, with what the fork said of the text.
fn said(flow: &str, at: &str, became: &[(&cases::Case, Became)]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "### Cell cases of `{flow}` at {}\n", &at[..at.len().min(9)]);
    let _ = writeln!(out, "| Case | Before | After | Cost |\n|---|---|---|---|");
    let mut details = String::new();
    let (mut ran, mut cost) = (0, 0.0);
    for (case, how) in became {
        let row = match how {
            Became::Ran(kept) | Became::Stood(kept) => {
                let key = kept.tries.last().map(|t| t.key.clone()).unwrap_or_default();
                let after: Vec<&Try> = kept.tries.iter().filter(|t| t.key == key).collect();
                let before_key = (kept.tries.iter()).rev().find(|t| t.key != key).map(|t| t.key.clone());
                let before: Vec<&Try> = kept
                    .tries
                    .iter()
                    .filter(|t| Some(&t.key) == before_key.as_ref())
                    .collect();
                let spent: f64 = after.iter().map(|t| t.cost_usd).sum();
                if matches!(how, Became::Ran(_)) {
                    ran += 1;
                    cost += spent;
                }
                if after.iter().any(|t| !t.passed) {
                    let _ = writeln!(details, "\n#### {} {}\n", case.id, verdict(&after));
                    for (n, t) in after.iter().enumerate().filter(|(_, t)| !t.passed) {
                        let _ = writeln!(details, "Try {}:", n + 1);
                        for found in &t.mechanical {
                            let _ = writeln!(details, "- {found}");
                        }
                        if let Some((false, why)) = &t.reading {
                            let _ = writeln!(details, "- Reading: {why}");
                        }
                        if let Some((cached, fresh)) = t.fork_cache
                            && cached < 4 * fresh
                        {
                            let _ = writeln!(
                                details,
                                "- The fork read {cached} of its first request's {} tokens from the session's cache, \
                                 not the bulk of it: its request did not begin as the session's did.",
                                cached + fresh
                            );
                        }
                        if let Some(fork) = t.fork.as_ref().filter(|f| !f.trim().is_empty()) {
                            let fork: String = fork.split_whitespace().collect::<Vec<_>>().join(" ");
                            let short: String = fork.chars().take(1200).collect();
                            let _ = writeln!(details, "- The fork about the text: {short}");
                        }
                    }
                }
                let stood = if matches!(how, Became::Stood(_)) { " (kept)" } else { "" };
                format!("{}{stood} | {} | ${spent:.2}", verdict(&before), verdict(&after))
            }
            Became::Planned(kept) => {
                let last = kept.tries.last().map(|t| t.cost_usd);
                let cost = last.map_or_else(|| "cost unknown".to_string(), |c| format!("about ${c:.2}"));
                format!("— | would run ({cost}) | —")
            }
            Became::Refused(why) => format!("— | refused: {why} | —"),
            Became::Unspent(why) => format!("— | not run: {why} | —"),
        };
        let _ = writeln!(out, "| {} | {row} |", case.id);
    }
    let _ = writeln!(out, "\n{ran} cases ran, for ${cost:.2}.");
    out.push_str(&details);
    out
}
