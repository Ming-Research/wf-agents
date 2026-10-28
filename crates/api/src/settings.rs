//! The settings the owner keeps on the page: the tiers' models, the highest tier an item may be at, the architect's
//! tier and the session manager's settings, read from the configuration the engine runs with and written to the
//! installation's file in the data directory, its comments kept, and committed there; and an item's tier, changed in
//! the database as the owner's.

use std::path::Path;
use std::process::Command;

use config::toml::Value;
use db::rusqlite::params;
use db::{Actor, Db};

use crate::{Settings, SettingsChange, Tier};

/// The settings as the configuration's last commit has them.
pub fn settings(data: &Path) -> Result<Settings, String> {
    let c = config::committed(data).map_err(|e| e.to_string())?;
    let agents = &c.config.install.agents;
    let mut names: Vec<&String> = agents.levels.keys().collect();
    names.sort_by_key(|n| (n.parse::<i64>().ok(), (*n).clone()));
    Ok(Settings {
        tiers: names
            .into_iter()
            .map(|n| Tier {
                name: n.clone(),
                model: agents.levels[n].model.clone(),
                desc: agents.levels[n].desc.clone(),
            })
            .collect(),
        highest_level: c.config.install.items.highest_level.clone(),
        architect: c.config.architect_level(),
        context_limit: agents.context_limit as i64,
        keep_warm_minutes: sessions::warm::AFTER_MS / 60_000,
        keep_warm_most: sessions::warm::MOST as i64,
        commit: c.commit,
    })
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

/// Writes a change to the settings into the installation's file, its comments kept, and commits it in the data
/// directory, saying what the owner changed; the engine takes it from its next round. A change the configuration would
/// refuse, or one onto an installation's file with changes not committed, is refused before anything is written.
/// Gives what was changed, in words.
pub fn save_settings(data: &Path, change: &SettingsChange) -> Result<String, String> {
    let file = config::INSTALL_FILE;
    if !git(data, &["status", "--porcelain", "--", file])?.trim().is_empty() {
        return Err(format!(
            "{file} has changes not committed in the data directory: commit or drop them first"
        ));
    }
    let now = settings(data)?;
    let mut text = git(data, &["show", &format!("HEAD:{file}")])?;
    let mut said: Vec<String> = Vec::new();
    for t in &now.tiers {
        if let Some(m) = change.models.get(&t.name).filter(|m| **m != t.model) {
            let m = m.trim();
            if m.is_empty() {
                return Err(format!("tier {} needs a model", t.name));
            }
            let table = format!("agents.levels.{}", t.name);
            text = config::set_setting(&text, &table, "model", Some(&Value::String(m.to_string())));
            said.push(format!("tier {} runs {m} instead of {}", t.name, t.model));
        }
    }
    if let Some(name) = change.models.keys().find(|n| !now.tiers.iter().any(|t| t.name == **n)) {
        return Err(format!("there is no tier {name}"));
    }
    if let Some(h) = &change.highest_level
        && Some(h) != now.highest_level.as_ref()
    {
        text = config::set_setting(&text, "items", "highest_level", Some(&Value::String(h.clone())));
        said.push(format!("items may be at tier {h} at the highest"));
    }
    if let Some(a) = &change.architect
        && *a != now.architect
    {
        text = config::set_setting(&text, "agents", "architect", Some(&Value::String(a.clone())));
        said.push(format!("the architect runs at tier {a}"));
    }
    if let Some(n) = change.context_limit
        && n != now.context_limit
    {
        if n <= 0 {
            return Err("the session size limit is a number of tokens above 0".into());
        }
        text = config::set_setting(&text, "agents", "context_limit", Some(&Value::Integer(n)));
        said.push(format!("a session past {n} tokens is replaced"));
    }
    if said.is_empty() {
        return Ok("nothing changed".into());
    }
    // What the configuration would say of it against the flow built into this program, before anything is written.
    let c = config::with_install(data, &text).map_err(|e| e.to_string())?;
    let relations = db::relations(db::SCHEMA).map_err(|e| e.to_string())?;
    let tables = db::index_of(db::SCHEMA).map_err(|e| e.to_string())?;
    let compiled = flow::compile(flow::FLOW, &flow::Context { relations, tables }).map_err(|e| e.to_string())?;
    let problems = c.check(&compiled);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::write(data.join(file), &text).map_err(|e| format!("{file}: {e}"))?;
    let message = format!("The owner changed the settings on the page: {}", said.join("; "));
    git(data, &["add", "--", file])?;
    if let Err(e) = git(data, &["commit", "-q", "-m", &message, "--", file]) {
        // Nothing half done is left: the file goes back to what is committed.
        let _ = git(data, &["checkout", "--", file]);
        return Err(e);
    }
    Ok(said.join("; "))
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

    /// A data directory whose configuration the flow takes, with two tiers.
    fn data(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("projects")).unwrap();
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        let install = "# The installation.\n[owner]\nlanguage = \"English\"\n[items]\ntitle_max = 40\n[machines]\n\
                       workstation = \"here\"\nheavy = \"builds\"\npoll_interval = 60\n[machines.hosts.here]\nslots = 1\n\
                       [agents.clis.claude]\nkind = \"claude\"\ncommand = \"claude\"\n[agents.levels.1]\n\
                       desc = \"small\"\ncli = \"claude\"\n# The small model.\nmodel = \"m-one\"\n[agents.levels.2]\n\
                       desc = \"large\"\ncli = \"claude\"\nmodel = \"m-two\"\n[gh]\nallow = [\"pr view\"]\n[query]\n\
                       max_rows = 10\ntimeout_ms = 1000\n";
        std::fs::write(dir.join("install.toml"), install).unwrap();
        let project = "repo = \"o/p\"\ncheckout = \"/tmp\"\ndesign_goal = \"simple\"\ndesign_paths = [\"design/\"]\n\
                       test_paths = [\"tests/\"]\nrules = [\"ground\"]\n[ci]\nfast = \"ci\"\nfull = [\"ci\"]\n\
                       merge_checks = [\"check\"]\n";
        std::fs::write(dir.join("projects/p.toml"), project).unwrap();
        std::fs::write(dir.join("rules/ground.md"), "Facts first.\n").unwrap();
        run(&dir, &["init", "-q"]);
        // The settings commit as whoever the data directory's git says.
        run(&dir, &["config", "user.name", "t"]);
        run(&dir, &["config", "user.email", "t@t"]);
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-q", "-m", "configuration"]);
        dir
    }

    // A change the owner makes on the page is written into the installation's file, its comments kept, and committed
    // saying what the owner changed; one the configuration would refuse writes nothing.
    #[test]
    fn a_change_to_the_settings_is_committed_and_a_refused_one_writes_nothing() {
        let dir = data("save");
        let s = settings(&dir).unwrap();
        let models: Vec<&str> = s.tiers.iter().map(|t| t.model.as_str()).collect();
        assert_eq!(models, ["m-one", "m-two"]);
        assert_eq!((s.architect.as_str(), s.highest_level.as_deref()), ("2", None));
        let change = SettingsChange {
            models: [("2".to_string(), "m-three".to_string())].into(),
            highest_level: Some("1".into()),
            architect: None,
            context_limit: Some(400_000),
        };
        let said = save_settings(&dir, &change).unwrap();
        assert!(said.contains("tier 2 runs m-three instead of m-two"), "{said}");
        let text = std::fs::read_to_string(dir.join("install.toml")).unwrap();
        assert!(text.starts_with("# The installation.") && text.contains("# The small model.\nmodel = \"m-one\""));
        let log = git(&dir, &["log", "-1", "--format=%s"]).unwrap();
        assert!(
            log.starts_with("The owner changed the settings on the page: tier 2"),
            "{log}"
        );
        let s = settings(&dir).unwrap();
        assert_eq!(
            (s.tiers[1].model.as_str(), s.highest_level.as_deref(), s.context_limit),
            ("m-three", Some("1"), 400_000)
        );
        let refused = SettingsChange {
            models: Default::default(),
            highest_level: Some("9".into()),
            architect: None,
            context_limit: None,
        };
        let why = save_settings(&dir, &refused).unwrap_err();
        assert!(why.contains("items.highest_level names no level: 9"), "{why}");
        assert_eq!(std::fs::read_to_string(dir.join("install.toml")).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
