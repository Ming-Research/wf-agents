//! The settings the owner keeps on the page: the installation's and each project's, read from the configuration the
//! engine runs with. Those the owner changes there are written into the configuration's files in the data directory,
//! their comments and layout kept, and committed there, once the configuration they make reads, fits the flow and
//! shows each change as asked; so is the login account the agents run under, one of the CLI's configuration
//! directories beside the one in use, each read only for the account it is signed in as. An item's tier is changed in
//! the database as the owner's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use config::toml_edit::Value;
use db::rusqlite::params;
use db::{Actor, Db};
use sessions::choose::same_dir;

use crate::{
    Account, AccountChoice, CliAccounts, CliArgs, CounterLimit, HostSlots, ProjectChange, ProjectSettings, Settings,
    SettingsChange, Tag, Tier,
};

/// The least and the most a session's size limit may be, in tokens: a session must hold a cell's rules and inputs, and
/// the models' context holds a million.
const CONTEXT_LIMITS: (i64, i64) = (50_000, 1_000_000);

/// The most works a machine may run at once.
const MOST_SLOTS: i64 = 32;

/// The most a counter's limit may be; a limit is at least 1, since 0 would set none.
const MOST_LIMIT: i64 = 1_000_000;

/// The flow built into this program, compiled once: the counters a project's limits name, and what a change to the
/// configuration is checked against, as the engine checks the configuration it reads.
fn built_in() -> Result<&'static flow::Compiled, String> {
    static FLOW: OnceLock<Result<flow::Compiled, String>> = OnceLock::new();
    FLOW.get_or_init(|| {
        let relations = db::relations(db::SCHEMA).map_err(|e| e.to_string())?;
        let tables = db::index_of(db::SCHEMA).map_err(|e| e.to_string())?;
        flow::compile(flow::FLOW, &flow::Context { relations, tables }).map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// The settings as the configuration's last commit has them.
pub fn settings(data: &Path) -> Result<Settings, String> {
    let c = config::committed(data).map_err(|e| e.to_string())?;
    Ok(view(&c.config, &c.commit, built_in()?))
}

/// The counters of the flow, each with what it counts, in the flow's order and each once.
fn counters(compiled: &flow::Compiled) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for c in compiled.graphs.iter().flat_map(|g| &g.counters) {
        if !out.iter().any(|(n, _)| *n == c.name) {
            out.push((&c.name, &c.desc));
        }
    }
    out
}

/// The settings as a configuration has them, read from the commit given.
fn view(c: &config::Config, commit: &str, compiled: &flow::Compiled) -> Settings {
    let install = &c.install;
    let agents = &install.agents;
    let mut names: Vec<&String> = agents.levels.keys().collect();
    names.sort_by_key(|n| (n.parse::<i64>().ok(), (*n).clone()));
    let counters = counters(compiled);
    let projects = c
        .projects
        .iter()
        .map(|(name, p)| {
            let mut limits: Vec<CounterLimit> = counters
                .iter()
                .filter_map(|(n, desc)| {
                    p.limits.get(*n).map(|l| CounterLimit {
                        counter: n.to_string(),
                        limit: *l as i64,
                        desc: desc.to_string(),
                    })
                })
                .collect();
            limits.extend(
                p.limits
                    .iter()
                    .filter(|(n, _)| !counters.iter().any(|(c, _)| c == n))
                    .map(|(n, l)| CounterLimit {
                        counter: n.clone(),
                        limit: *l as i64,
                        desc: String::new(),
                    }),
            );
            ProjectSettings {
                name: name.clone(),
                limits,
                observation_tags: p
                    .observation_tags
                    .iter()
                    .map(|t| Tag {
                        name: t.name.clone(),
                        desc: t.desc.clone(),
                    })
                    .collect(),
                flow_feedback: p.flow_feedback,
                repo: p.repo.clone(),
                checkout: p.checkout.display().to_string(),
                main: p.main.clone(),
                design_paths: p.design_paths.clone(),
                test_paths: p.test_paths.clone(),
                design_check: p.design_check.clone(),
                ci_fast: p.ci.fast.clone(),
                ci_full: p.ci.full.clone(),
                merge_checks: p.ci.merge_checks.clone(),
                constitution: p.constitution.as_ref().map(|x| x.display().to_string()),
                rules: p.rules.clone(),
            }
        })
        .collect();
    let taken: BTreeSet<&String> = c.projects.values().flat_map(|p| &p.rules).collect();
    Settings {
        tiers: names
            .into_iter()
            .map(|n| Tier {
                name: n.clone(),
                model: agents.levels[n].model.clone(),
                desc: agents.levels[n].desc.clone(),
                // A model's write of the cache is the same number of its reads whatever the prompt's size.
                keep_warm_most: sessions::warm::most(&agents.levels[n].model, 0) as i64,
            })
            .collect(),
        highest_level: install.items.highest_level.clone(),
        architect: c.architect_level(),
        context_limit: agents.context_limit as i64,
        keep_warm_minutes: sessions::warm::AFTER_MS / 60_000,
        language: install.owner.language.clone(),
        hosts: install
            .machines
            .hosts
            .iter()
            .map(|(name, h)| HostSlots {
                name: name.clone(),
                slots: i64::from(h.slots),
            })
            .collect(),
        clis: agents
            .clis
            .iter()
            .map(|(name, x)| CliArgs {
                name: name.clone(),
                command: x.command.clone(),
                args: x.args.clone(),
            })
            .collect(),
        gh_allow: install.gh.allow.clone(),
        query_max_rows: i64::from(install.query.max_rows),
        query_timeout_ms: install.query.timeout_ms as i64,
        projects,
        rules: c
            .rules
            .iter()
            .filter(|(n, _)| taken.contains(n))
            .map(|(n, t)| (n.clone(), t.clone()))
            .collect(),
        commit: commit.to_string(),
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// One setting to write: the file of the configuration it is in, the keys that lead to it there, and its new value.
struct Setting {
    file: String,
    path: Vec<String>,
    value: Value,
}

impl Setting {
    fn new(file: &str, path: &[&str], value: impl Into<Value>) -> Setting {
        Setting {
            file: file.to_string(),
            path: path.iter().map(|p| p.to_string()).collect(),
            value: value.into(),
        }
    }
}

/// Writes settings into the configuration's files and commits them in the data directory with the message given, one
/// writer at a time. It writes nothing unless the configuration they make reads, fits the flow built into this
/// program as the engine checks it, and shows what `unmet` looks for (each problem it gives refuses the change); and
/// nothing onto a file with changes not committed. A commit that fails puts the files back as they were committed.
fn write(
    data: &Path,
    settings: &[Setting],
    message: &str,
    unmet: &dyn Fn(&config::Config, &str) -> Vec<String>,
) -> Result<(), String> {
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    for s in settings {
        if !texts.contains_key(&s.file) {
            if !git(data, &["status", "--porcelain", "--", &s.file])?.trim().is_empty() {
                return Err(format!(
                    "{} has changes not committed in the data directory: commit or drop them first",
                    s.file
                ));
            }
            let text = git(data, &["show", &format!("HEAD:{}", s.file)])?;
            texts.insert(s.file.clone(), text);
        }
        let path: Vec<&str> = s.path.iter().map(String::as_str).collect();
        let text = &texts[&s.file];
        let changed = config::set_setting(text, &path, Some(s.value.clone()))
            .map_err(|e| format!("{}: {}: {e}", s.file, s.path.join(".")))?;
        texts.insert(s.file.clone(), changed);
    }
    let c = config::with_files(data, &texts).map_err(|e| e.to_string())?;
    let mut problems = c.config.check(built_in()?);
    problems.extend(unmet(&c.config, &c.commit));
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let files: Vec<&str> = texts.keys().map(String::as_str).collect();
    for (file, text) in &texts {
        std::fs::write(data.join(file), text).map_err(|e| format!("{file}: {e}"))?;
    }
    let mut add = vec!["add", "--"];
    add.extend(&files);
    let mut commit = vec!["commit", "-q", "-m", message, "--"];
    commit.extend(&files);
    if let Err(e) = git(data, &add).and_then(|_| git(data, &commit)) {
        // Nothing half done is left: the files go back to what is committed, in the index too.
        let mut back = vec!["checkout", "HEAD", "--"];
        back.extend(&files);
        let _ = git(data, &back);
        return Err(e);
    }
    Ok(())
}

/// One writer of the configuration at a time: two saves at once would each read the commit the other replaces.
fn writing() -> std::sync::MutexGuard<'static, ()> {
    static WRITING: Mutex<()> = Mutex::new(());
    WRITING.lock().unwrap_or_else(|e| e.into_inner())
}

/// A text on one line, no longer than `most` characters, with nothing round it: what a name or a line of the
/// configuration holds.
fn one_line(text: &str, most: usize) -> bool {
    !text.is_empty() && text.trim() == text && !text.contains(['\n', '\r']) && text.chars().count() <= most
}

/// Writes a change to the settings into the configuration's files, the installation's and the projects' own, their
/// comments and layout kept, and commits it in the data directory, saying what the owner changed; the engine takes it
/// from its next round. A value of the wrong kind or out of its range, a model the tier's CLI does not take, or a
/// change the configuration would refuse is refused before anything is written, saying why. Gives what was changed,
/// in words.
pub fn save_settings(data: &Path, change: &SettingsChange) -> Result<String, String> {
    let _writing = writing();
    let c = config::committed(data).map_err(|e| e.to_string())?;
    let now = view(&c.config, &c.commit, built_in()?);
    let install = config::INSTALL_FILE;
    let mut settings: Vec<Setting> = Vec::new();
    let mut said: Vec<String> = Vec::new();
    for t in &now.tiers {
        let Some(m) = change.models.get(&t.name).map(|m| m.trim()).filter(|m| *m != t.model) else {
            continue;
        };
        if m.is_empty() {
            return Err(format!("tier {} needs a model", t.name));
        }
        let (_, cli) = c
            .config
            .level(&t.name)
            .ok_or_else(|| format!("tier {} runs in no CLI", t.name))?;
        if !cli.kind.knows_model(m) {
            return Err(format!(
                "tier {}: {} takes no model {m:?}; a model is a family's alias, such as opus, or a model's full name, \
                 such as claude-opus-5-5",
                t.name, cli.command
            ));
        }
        settings.push(Setting::new(install, &["agents", "levels", &t.name, "model"], m));
        said.push(format!("tier {} runs {m} instead of {}", t.name, t.model));
    }
    if let Some(name) = change.models.keys().find(|n| !now.tiers.iter().any(|t| t.name == **n)) {
        return Err(format!("there is no tier {name}"));
    }
    let tier = |n: &str| {
        if now.tiers.iter().any(|t| t.name == n) {
            Ok(())
        } else {
            Err(format!("there is no tier {n}"))
        }
    };
    if let Some(h) = &change.highest_level
        && Some(h) != now.highest_level.as_ref()
    {
        tier(h)?;
        settings.push(Setting::new(install, &["items", "highest_level"], h.as_str()));
        said.push(format!("items may be at tier {h} at the highest"));
    }
    if let Some(a) = &change.architect
        && *a != now.architect
    {
        tier(a)?;
        settings.push(Setting::new(install, &["agents", "architect"], a.as_str()));
        said.push(format!("the architect runs at tier {a}"));
    }
    if let Some(n) = change.context_limit
        && n != now.context_limit
    {
        let (least, most) = CONTEXT_LIMITS;
        if !(least..=most).contains(&n) {
            return Err(format!(
                "the session size limit is a number of tokens from {least} to {most}"
            ));
        }
        settings.push(Setting::new(install, &["agents", "context_limit"], n));
        said.push(format!("a session past {n} tokens is replaced"));
    }
    if let Some(l) = change.language.as_deref().map(str::trim)
        && l != now.language
    {
        if !one_line(l, 40) {
            return Err("the owner's language is a name on one line, such as English".into());
        }
        settings.push(Setting::new(install, &["owner", "language"], l));
        said.push(format!("the owner reads {l} instead of {}", now.language));
    }
    for (host, n) in &change.slots {
        let Some(h) = now.hosts.iter().find(|h| h.name == *host) else {
            return Err(format!("there is no machine {host}"));
        };
        if *n == h.slots {
            continue;
        }
        if !(0..=MOST_SLOTS).contains(n) {
            return Err(format!("machine {host} runs from 0 to {MOST_SLOTS} works at once"));
        }
        settings.push(Setting::new(install, &["machines", "hosts", host, "slots"], *n));
        said.push(format!("machine {host} runs {n} works at once instead of {}", h.slots));
    }
    for (name, p) in &change.projects {
        let Some(before) = now.projects.iter().find(|x| x.name == *name) else {
            return Err(format!("there is no project {name}"));
        };
        project_change(name, before, p, built_in()?, &mut settings, &mut said)?;
    }
    if said.is_empty() {
        return Ok("nothing changed".into());
    }
    let message = format!("The owner changed the settings on the page: {}", said.join("; "));
    let compiled = built_in()?;
    write(data, &settings, &message, &|config, commit| {
        unmet(change, &view(config, commit, compiled))
    })?;
    Ok(said.join("; "))
}

/// What of a change to a project's settings is to be written in its own file, and said, each value checked first.
fn project_change(
    name: &str,
    before: &ProjectSettings,
    p: &ProjectChange,
    compiled: &flow::Compiled,
    settings: &mut Vec<Setting>,
    said: &mut Vec<String>,
) -> Result<(), String> {
    let file = config::project_file(name);
    let counters = counters(compiled);
    for (counter, n) in &p.limits {
        let was = before.limits.iter().find(|l| l.counter == *counter).map(|l| l.limit);
        if was == Some(*n) {
            continue;
        }
        if !counters.iter().any(|(c, _)| c == counter) {
            return Err(format!("project {name}: the flow has no counter {counter}"));
        }
        if !(1..=MOST_LIMIT).contains(n) {
            return Err(format!(
                "project {name}: the limit of {counter} is a whole number from 1 to {MOST_LIMIT}"
            ));
        }
        settings.push(Setting::new(&file, &["limits", counter], *n));
        said.push(match was {
            Some(w) => format!("project {name} limits {counter} to {n} instead of {w}"),
            None => format!("project {name} limits {counter} to {n}"),
        });
    }
    if let Some(tags) = &p.observation_tags {
        let tags: Vec<Tag> = tags
            .iter()
            .map(|t| Tag {
                name: t.name.trim().to_string(),
                desc: t.desc.trim().to_string(),
            })
            .collect();
        if tags != before.observation_tags {
            let mut seen = BTreeSet::new();
            for t in &tags {
                if !one_line(&t.name, 40) || t.name.contains(char::is_whitespace) {
                    return Err(format!(
                        "project {name}: an observation tag's name is one word with no spaces: {:?}",
                        t.name
                    ));
                }
                if !seen.insert(t.name.as_str()) {
                    return Err(format!("project {name}: the observation tag {} is there twice", t.name));
                }
                if !one_line(&t.desc, 200) {
                    return Err(format!(
                        "project {name}: the observation tag {} says what it covers on one line",
                        t.name
                    ));
                }
            }
            let rows: Vec<Vec<(&str, &str)>> = tags
                .iter()
                .map(|t| vec![("name", t.name.as_str()), ("desc", t.desc.as_str())])
                .collect();
            settings.push(Setting::new(&file, &["observation_tags"], config::table_list(&rows)));
            said.push(format!(
                "project {name}'s observation tags are {}",
                tag_change(&before.observation_tags, &tags)
            ));
        }
    }
    if let Some(on) = p.flow_feedback
        && on != before.flow_feedback
    {
        settings.push(Setting::new(&file, &["flow_feedback"], on));
        said.push(if on {
            format!("project {name}'s agents hand in flow feedback")
        } else {
            format!("project {name}'s agents hand in no flow feedback")
        });
    }
    Ok(())
}

/// How a project's tags changed, in words: those added, those taken out and those that say anew what they cover.
fn tag_change(before: &[Tag], after: &[Tag]) -> String {
    let names = |ts: &[Tag]| ts.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    let added: Vec<String> = names(after)
        .into_iter()
        .filter(|n| !before.iter().any(|t| t.name == *n))
        .collect();
    let gone: Vec<String> = names(before)
        .into_iter()
        .filter(|n| !after.iter().any(|t| t.name == *n))
        .collect();
    let described: Vec<String> = after
        .iter()
        .filter(|t| before.iter().any(|b| b.name == t.name && b.desc != t.desc))
        .map(|t| t.name.clone())
        .collect();
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("{} added", added.join(", ")));
    }
    if !gone.is_empty() {
        parts.push(format!("{} taken out", gone.join(", ")));
    }
    if !described.is_empty() {
        parts.push(format!("{} described anew", described.join(", ")));
    }
    if parts.is_empty() {
        parts.push("put in another order".into());
    }
    parts.join(", ")
}

/// What of a change the settings a configuration makes do not show: each value asked that they do not hold.
fn unmet(change: &SettingsChange, after: &Settings) -> Vec<String> {
    let mut out = Vec::new();
    let mut not = |what: String| out.push(format!("{what} would not be as asked"));
    for (tier, m) in &change.models {
        if !after.tiers.iter().any(|t| t.name == *tier && t.model == m.trim()) {
            not(format!("tier {tier}'s model"));
        }
    }
    if change
        .highest_level
        .as_ref()
        .is_some_and(|h| after.highest_level.as_ref() != Some(h))
    {
        not("the highest tier of an item".into());
    }
    if change.architect.as_ref().is_some_and(|a| after.architect != *a) {
        not("the architect's tier".into());
    }
    if change.context_limit.is_some_and(|n| after.context_limit != n) {
        not("the session size limit".into());
    }
    if change.language.as_deref().is_some_and(|l| after.language != l.trim()) {
        not("the owner's language".into());
    }
    for (host, n) in &change.slots {
        if !after.hosts.iter().any(|h| h.name == *host && h.slots == *n) {
            not(format!("machine {host}'s slots"));
        }
    }
    for (name, p) in &change.projects {
        let Some(a) = after.projects.iter().find(|x| x.name == *name) else {
            not(format!("project {name}"));
            continue;
        };
        for (counter, n) in &p.limits {
            if !a.limits.iter().any(|l| l.counter == *counter && l.limit == *n) {
                not(format!("project {name}'s limit of {counter}"));
            }
        }
        let tags_differ = p.observation_tags.as_ref().is_some_and(|tags| {
            tags.len() != a.observation_tags.len()
                || tags
                    .iter()
                    .zip(&a.observation_tags)
                    .any(|(x, y)| x.name.trim() != y.name || x.desc.trim() != y.desc)
        });
        if tags_differ {
            not(format!("project {name}'s observation tags"));
        }
        if p.flow_feedback.is_some_and(|f| a.flow_feedback != f) {
            not(format!("project {name}'s flow feedback"));
        }
    }
    out
}

/// The login accounts the agents of each CLI may run under: the CLI's configuration directories in the directory of
/// the one it runs with now, named by the CLI's name alone or by it, a dash and the account's own name, each with the
/// account it is signed in as, which is all that is read of it; the one in use is listed even before it is made.
pub fn accounts(data: &Path) -> Result<Vec<CliAccounts>, String> {
    let c = config::committed(data).map_err(|e| e.to_string())?;
    Ok(c.config
        .install
        .agents
        .clis
        .iter()
        .map(|(name, cli)| cli_accounts(data, name, cli))
        .collect())
}

fn cli_accounts(data: &Path, name: &str, cli: &config::Cli) -> CliAccounts {
    let used = cli.home(data, name);
    let dir = used.parent().map_or_else(|| data.join("agents"), Path::to_path_buf);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let named = n == name || n.strip_prefix(name).is_some_and(|r| r.len() > 1 && r.starts_with('-'));
            if named && e.path().is_dir() {
                dirs.push(e.path());
            }
        }
    }
    if !dirs.iter().any(|d| same_dir(d, &used)) {
        dirs.push(used.clone());
    }
    dirs.sort();
    let (env, login) = sessions::cli::sign_in(cli.kind);
    CliAccounts {
        cli: name.to_string(),
        command: cli.command.clone(),
        dir: dir.display().to_string(),
        env: env.to_string(),
        login: login.to_string(),
        accounts: dirs
            .iter()
            .map(|d| Account {
                name: d
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                path: d.display().to_string(),
                email: sessions::cli::signed_in_as(cli.kind, d),
                used: same_dir(d, &used),
            })
            .collect(),
    }
}

/// Has the agents of a CLI run under another of its login accounts: writes the account's directory as the CLI's
/// `config_dir` in the installation's file (by its path from the data directory when it is inside it) and commits it,
/// saying so; the engine takes it from its next round, where every line starts a new session, since a session is kept
/// in the directory it ran in. An account that is not signed in is refused, saying how to sign it in.
pub fn switch_account(data: &Path, choice: &AccountChoice) -> Result<String, String> {
    let _writing = writing();
    let c = config::committed(data).map_err(|e| e.to_string())?;
    let cli = c
        .config
        .install
        .agents
        .clis
        .get(&choice.cli)
        .ok_or_else(|| format!("there is no CLI {}", choice.cli))?;
    let all = cli_accounts(data, &choice.cli, cli);
    let to = all
        .accounts
        .iter()
        .find(|a| a.name == choice.name)
        .ok_or_else(|| format!("there is no login account {} in {}", choice.name, all.dir))?;
    if to.used {
        return Ok(format!("the agents run under {} already", to.name));
    }
    if to.email.is_none() {
        return Err(format!(
            "{} is not signed in: run `{}={} {}` and type {} there first",
            to.name, all.env, to.path, all.command, all.login
        ));
    }
    let from = all
        .accounts
        .iter()
        .find(|a| a.used)
        .map_or_else(|| cli.home(data, &choice.cli).display().to_string(), |a| a.name.clone());
    let target = PathBuf::from(&to.path);
    let value = target
        .strip_prefix(data)
        .map_or_else(|_| to.path.clone(), |p| p.display().to_string());
    let said = format!(
        "{} runs the agents under the login in {} instead of {}",
        choice.cli, to.name, from
    );
    let setting = Setting::new(
        config::INSTALL_FILE,
        &["agents", "clis", &choice.cli, "config_dir"],
        value.as_str(),
    );
    let message = format!("The owner switched the agents' login account on the page: {said}");
    write(data, &[setting], &message, &|config, _| {
        let home = config
            .install
            .agents
            .clis
            .get(&choice.cli)
            .map(|x| x.home(data, &choice.cli));
        if home.is_some_and(|h| same_dir(&h, &target)) {
            Vec::new()
        } else {
            vec![format!(
                "{}'s configuration directory would not be {}",
                choice.cli, to.path
            )]
        }
    })?;
    Ok(said)
}

/// Puts an item at another tier, as the owner: a tier the configuration has, at most the highest an item may be at.
/// The change is the owner's in the item's history, and its works run at the new tier from their next round.
pub fn set_level(db: &mut Db, data: &Path, item: i64, level: &str) -> Result<String, String> {
    let c = config::committed(data).map_err(|e| e.to_string())?.config;
    if c.level(level).is_none() {
        return Err(format!("there is no tier {level}"));
    }
    c.item_level(level)?;
    let changed = db
        .write(Actor::Owner, |tx| {
            Ok(tx.execute(
                "UPDATE items SET level = ?2 WHERE id = ?1 AND level <> ?2",
                params![item, level],
            )?)
        })
        .map_err(|e| e.to_string())?;
    Ok(if changed > 0 {
        format!("item {item} is at tier {level}")
    } else {
        format!("item {item} was at tier {level} already")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "{args:?}");
    }

    /// A data directory whose configuration the flow takes, with two tiers and a project whose file has comments.
    fn data(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("projects")).unwrap();
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        let install = "# The installation.\n[owner]\nlanguage = \"English\"\n[items]\ntitle_max = 40\n[machines]\n\
                       workstation = \"here\"\nheavy = \"builds\"\npoll_interval = 60\n[machines.hosts.here]\nslots = 1\n\
                       [agents.clis.claude]\nkind = \"claude\"\ncommand = \"claude\"\n[agents.levels.1]\n\
                       desc = \"small\"\ncli = \"claude\"\n# The small model.\nmodel = \"claude-haiku-5-5\"\n\
                       [agents.levels.2]\ndesc = \"large\"\ncli = \"claude\"\nmodel = \"claude-sonnet-5-5\"\n[gh]\n\
                       allow = [\"pr view\"]\n[query]\nmax_rows = 10\ntimeout_ms = 1000\n";
        std::fs::write(dir.join("install.toml"), install).unwrap();
        let project = "repo = \"o/p\"\ncheckout = \"/tmp\"\ndesign_goal = \"simple\"\ndesign_paths = [\"design/\"]\n\
                       test_paths = [\"tests/\"]\nrules = [\"ground\"]\nconstitution = \"CONSTITUTION.md\"\n\
                       # The tags, for the owner to change.\nobservation_tags = [\n  \
                       { name = \"ui\", desc = \"What the owner sees\" },\n]\n\n[ci]\nfast = \"ci\"\nfull = [\"ci\"]\n\
                       merge_checks = [\"check\"]\n\n[limits]\n# Asks are cheap.\nasks = 10\nfixes = 6\n";
        std::fs::write(dir.join("projects/p.toml"), project).unwrap();
        std::fs::write(dir.join("rules/ground.md"), "Facts first.\n").unwrap();
        std::fs::write(dir.join("rules/unused.md"), "Taken by no project.\n").unwrap();
        run(&dir, &["init", "-q"]);
        // The settings commit as whoever the data directory's git says.
        run(&dir, &["config", "user.name", "t"]);
        run(&dir, &["config", "user.email", "t@t"]);
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-q", "-m", "configuration"]);
        dir
    }

    fn last_message(dir: &Path) -> String {
        git(dir, &["log", "-1", "--format=%s"]).unwrap()
    }

    fn unchanged() -> SettingsChange {
        SettingsChange {
            models: Default::default(),
            highest_level: None,
            architect: None,
            context_limit: None,
            language: None,
            slots: Default::default(),
            projects: Default::default(),
        }
    }

    // The settings show the installation's and each project's, the rules the projects take in, and what each of the
    // flow's counters the project limits counts.
    #[test]
    fn the_settings_show_the_installation_and_each_project() {
        let dir = data("show");
        let s = settings(&dir).unwrap();
        let models: Vec<&str> = s.tiers.iter().map(|t| t.model.as_str()).collect();
        assert_eq!(models, ["claude-haiku-5-5", "claude-sonnet-5-5"]);
        assert_eq!((s.architect.as_str(), s.highest_level.as_deref()), ("2", None));
        assert_eq!(s.language, "English");
        assert_eq!(
            s.hosts,
            [HostSlots {
                name: "here".into(),
                slots: 1
            }]
        );
        assert_eq!((s.gh_allow.len(), s.query_max_rows, s.query_timeout_ms), (1, 10, 1000));
        assert_eq!(s.rules.keys().collect::<Vec<_>>(), ["ground"]);
        let p = &s.projects[0];
        assert_eq!(
            (p.name.as_str(), p.constitution.as_deref()),
            ("p", Some("CONSTITUTION.md"))
        );
        let limits: Vec<(&str, i64)> = p.limits.iter().map(|l| (l.counter.as_str(), l.limit)).collect();
        assert_eq!(limits, [("asks", 10), ("fixes", 6)]);
        assert!(p.limits.iter().all(|l| !l.desc.is_empty()));
        assert_eq!(p.observation_tags[0].name, "ui");
        assert!(p.flow_feedback);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A change the owner makes on the page is written into the installation's file and the project's own, their
    // comments kept, and committed saying what the owner changed; one the configuration would refuse writes nothing.
    #[test]
    fn a_change_to_the_settings_is_committed_and_a_refused_one_writes_nothing() {
        let dir = data("save");
        let change = SettingsChange {
            models: [("2".to_string(), "claude-opus-5-5".to_string())].into(),
            highest_level: Some("1".into()),
            context_limit: Some(400_000),
            language: Some("Chinese".into()),
            slots: [("here".to_string(), 3)].into(),
            projects: [(
                "p".to_string(),
                ProjectChange {
                    limits: [("asks".to_string(), 12)].into(),
                    observation_tags: Some(vec![
                        Tag {
                            name: "ui".into(),
                            desc: "What the owner sees".into(),
                        },
                        Tag {
                            name: " cost ".into(),
                            desc: "Tokens spent".into(),
                        },
                    ]),
                    flow_feedback: Some(false),
                },
            )]
            .into(),
            ..unchanged()
        };
        let said = save_settings(&dir, &change).unwrap();
        assert!(
            said.contains("tier 2 runs claude-opus-5-5 instead of claude-sonnet-5-5"),
            "{said}"
        );
        assert!(said.contains("project p limits asks to 12 instead of 10"), "{said}");
        assert!(said.contains("project p's observation tags are cost added"), "{said}");
        let install = std::fs::read_to_string(dir.join("install.toml")).unwrap();
        assert!(
            install.starts_with("# The installation.")
                && install.contains("# The small model.\nmodel = \"claude-haiku")
        );
        let project = std::fs::read_to_string(dir.join("projects/p.toml")).unwrap();
        assert!(
            project.contains("# Asks are cheap.\nasks = 12\nfixes = 6\n"),
            "{project}"
        );
        assert!(
            project.contains(
                "# The tags, for the owner to change.\nobservation_tags = [\n  { name = \"ui\", desc = \"What the owner \
                 sees\" },\n  { name = \"cost\", desc = \"Tokens spent\" },\n]\n"
            ),
            "{project}"
        );
        assert!(project.contains("flow_feedback = false\n"), "{project}");
        assert!(
            last_message(&dir).starts_with("The owner changed the settings on the page: tier 2"),
            "{}",
            last_message(&dir)
        );
        let s = settings(&dir).unwrap();
        assert_eq!(
            (s.tiers[1].model.as_str(), s.highest_level.as_deref(), s.context_limit),
            ("claude-opus-5-5", Some("1"), 400_000)
        );
        assert_eq!((s.language.as_str(), s.hosts[0].slots), ("Chinese", 3));
        assert_eq!(s.projects[0].observation_tags[1].name, "cost");
        assert!(!s.projects[0].flow_feedback);
        assert_eq!(save_settings(&dir, &change).unwrap(), "nothing changed");
        // Each value of the wrong kind or out of its range is refused, saying why, and nothing is written.
        let head = git(&dir, &["rev-parse", "HEAD"]).unwrap();
        let project_with = |p: ProjectChange| SettingsChange {
            projects: [("p".to_string(), p)].into(),
            ..unchanged()
        };
        let tags = |names: &[&str]| {
            Some(
                names
                    .iter()
                    .map(|n| Tag {
                        name: n.to_string(),
                        desc: "covers".into(),
                    })
                    .collect(),
            )
        };
        for (refused, why) in [
            (
                SettingsChange {
                    highest_level: Some("9".into()),
                    ..unchanged()
                },
                "there is no tier 9",
            ),
            (
                SettingsChange {
                    models: [("1".to_string(), "claude-sonet-5-5".to_string())].into(),
                    ..unchanged()
                },
                "claude takes no model \"claude-sonet-5-5\"",
            ),
            (
                SettingsChange {
                    context_limit: Some(10),
                    ..unchanged()
                },
                "from 50000 to 1000000",
            ),
            (
                SettingsChange {
                    slots: [("here".to_string(), 99)].into(),
                    ..unchanged()
                },
                "machine here runs from 0 to 32",
            ),
            (
                SettingsChange {
                    language: Some("".into()),
                    ..unchanged()
                },
                "a name on one line",
            ),
            (
                project_with(ProjectChange {
                    limits: [("asks".to_string(), 0)].into(),
                    ..Default::default()
                }),
                "the limit of asks is a whole number from 1",
            ),
            (
                project_with(ProjectChange {
                    limits: [("nope".to_string(), 3)].into(),
                    ..Default::default()
                }),
                "the flow has no counter nope",
            ),
            (
                project_with(ProjectChange {
                    observation_tags: tags(&["ui", "ui"]),
                    ..Default::default()
                }),
                "the observation tag ui is there twice",
            ),
            (
                project_with(ProjectChange {
                    observation_tags: tags(&["two words"]),
                    ..Default::default()
                }),
                "one word with no spaces",
            ),
        ] {
            let e = save_settings(&dir, &refused).unwrap_err();
            assert!(e.contains(why), "{e}");
        }
        assert_eq!(git(&dir, &["rev-parse", "HEAD"]).unwrap(), head);
        assert!(git(&dir, &["status", "--porcelain"]).unwrap().is_empty());
        // A file with changes not committed is not written over.
        std::fs::write(dir.join("projects/p.toml"), "# Being edited.\n").unwrap();
        let e = save_settings(
            &dir,
            &project_with(ProjectChange {
                flow_feedback: Some(true),
                ..Default::default()
            }),
        )
        .unwrap_err();
        assert!(e.contains("projects/p.toml has changes not committed"), "{e}");
        assert_eq!(
            std::fs::read_to_string(dir.join("projects/p.toml")).unwrap(),
            "# Being edited.\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The accounts are the CLI's configuration directories beside the one in use, each with the email it is signed in
    // as; switching to one signed in writes it as the CLI's directory and commits it, and one not signed in is refused.
    #[test]
    fn the_agents_switch_to_another_login_account() {
        let dir = data("accounts");
        let signed = |name: &str, email: &str| {
            let d = dir.join("agents").join(name);
            std::fs::create_dir_all(&d).unwrap();
            let json = format!("{{\"oauthAccount\": {{\"emailAddress\": \"{email}\", \"accountUuid\": \"u\"}}}}");
            std::fs::write(d.join(".claude.json"), json).unwrap();
        };
        signed("claude", "first@example.com");
        signed("claude-work", "work@example.com");
        std::fs::create_dir_all(dir.join("agents/claude-new")).unwrap();
        std::fs::create_dir_all(dir.join("agents/other")).unwrap();
        let all = accounts(&dir).unwrap();
        assert_eq!(all.len(), 1);
        let c = &all[0];
        assert_eq!(
            (c.cli.as_str(), c.env.as_str(), c.login.as_str()),
            ("claude", "CLAUDE_CONFIG_DIR", "/login")
        );
        let seen: Vec<(&str, Option<&str>, bool)> = c
            .accounts
            .iter()
            .map(|a| (a.name.as_str(), a.email.as_deref(), a.used))
            .collect();
        assert_eq!(
            seen,
            [
                ("claude", Some("first@example.com"), true),
                ("claude-new", None, false),
                ("claude-work", Some("work@example.com"), false)
            ]
        );
        let choose = |name: &str| AccountChoice {
            cli: "claude".into(),
            name: name.into(),
        };
        let e = switch_account(&dir, &choose("claude-new")).unwrap_err();
        assert!(
            e.contains("claude-new is not signed in: run `CLAUDE_CONFIG_DIR="),
            "{e}"
        );
        assert!(switch_account(&dir, &choose("other")).is_err());
        let said = switch_account(&dir, &choose("claude-work")).unwrap();
        assert_eq!(
            said,
            "claude runs the agents under the login in claude-work instead of claude"
        );
        let install = std::fs::read_to_string(dir.join("install.toml")).unwrap();
        assert!(
            install.contains("command = \"claude\"\nconfig_dir = \"agents/claude-work\"\n"),
            "{install}"
        );
        assert!(last_message(&dir).starts_with("The owner switched the agents' login account on the page"));
        let after = accounts(&dir).unwrap();
        let used: Vec<&str> = after[0]
            .accounts
            .iter()
            .filter(|a| a.used)
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(used, ["claude-work"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
