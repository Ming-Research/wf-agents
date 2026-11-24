//! What the engine finds in each project's declaration as it starts: `wf-agents.toml` on the project's main, read in a
//! git repository made for each project. The reference is the declaration's own rules (`design/decisions/engine/
//! settings.md`): a project whose declaration cannot be used is held with why, the engine and the other projects are not
//! stopped; and the declaration is the owner's to let change, so its `[protected]` paths name it, as the file says: a
//! path that ends in `/` is a directory and everything in it, any other is that file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use carry::Settings;
use carry::projects::{DeclarationText, Declared, declared};

const SAMPLE: &str = include_str!("../../install.sample.toml");

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The directory that holds the repository's flows.
fn flows() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The repository's own declaration, with the command that checks a project hosted in its local repository.
fn base() -> String {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wf-agents.toml"))
        .expect("the declaration is read");
    format!("{text}\n[local]\ncheck = \"true\"\n")
}

/// A repository whose main holds the files a declaration names, and the declaration's bytes where there are some.
fn repository(repo: &Path, declaration: Option<&[u8]>) {
    std::fs::create_dir_all(repo.join("design")).unwrap();
    git(repo, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Tester"),
        ("user.email", "tester@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        git(repo, &["config", key, value]);
    }
    for path in ["AGENTS.md", "design/constitution.md"] {
        std::fs::write(repo.join(path), "Some text.\n").unwrap();
    }
    if let Some(declaration) = declaration {
        std::fs::write(repo.join("wf-agents.toml"), declaration).unwrap();
    }
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "Files"]);
}

/// What the engine finds of one project of each name, hosted in its local repository holding that declaration on main.
fn found(dir: &Path, projects: &[(&str, Vec<u8>)]) -> (BTreeMap<String, Result<Declared, String>>, Vec<String>) {
    let mut settings = Settings::read(SAMPLE).expect("the sample reads");
    let sample = settings.projects["example"].clone();
    settings.projects.clear();
    for (name, declaration) in projects {
        let mut project = sample.clone();
        project.repo = dir.join(name);
        repository(&project.repo, Some(declaration));
        settings.projects.insert(name.to_string(), project);
    }
    let beside = flow::agents::read(&flows()).expect("the files beside the flows");
    declared(&settings, &beside, &[])
}

// A declaration is the owner's to let change, as it names commands the engine runs: it holds its project unless
// `[protected]` names it, as the file itself says what names a file: its own path, or a directory with a `/` at its end
// that holds it. A path that only looks like it, a longer name, a prefix of the name, or the name as a directory, names
// another file and protects nothing of it; nor does a declaration that protects nothing. A path that names it, or a
// declaration whose bytes are not text, or are none, or lines ended as Windows ends them, is each judged on its own.
// Each project is one git repository of a few files: about 0.3 s alone.
#[test]
fn a_declaration_is_carried_only_where_its_protected_paths_name_it_and_one_that_cannot_be_read_holds_its_project() {
    let dir = tempfile::tempdir().expect("a directory");
    let good = base();
    let listing = ", \"wf-agents.toml\"]";
    assert!(good.contains(listing), "the declaration lists itself");
    let protecting = |path: &str| good.replace(listing, &format!(", \"{path}\"]")).into_bytes();
    let none = good
        .lines()
        .filter(|line| *line != "[protected]" && !line.starts_with("paths = [\"AGENTS.md\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!none.contains("[protected]"), "{none}");
    let mut not_text = good.clone().into_bytes();
    not_text.extend_from_slice(b"\n# \xff\xfe\n");
    let crlf = good.replace('\n', "\r\n").into_bytes();
    let projects: Vec<(&str, Vec<u8>)> = vec![
        ("named", good.clone().into_bytes()),
        ("crlf", crlf),
        ("longer", protecting("wf-agents.toml.bak")),
        ("prefix", protecting("wf-agents")),
        ("directory", protecting("wf-agents.toml/")),
        ("other", protecting("flow/wf-agents.toml")),
        ("none", none.into_bytes()),
        ("empty", Vec::new()),
        ("not_text", not_text),
    ];
    let (found, faults) = found(dir.path(), &projects);
    assert_eq!(
        faults,
        Vec::<String>::new(),
        "a declaration is no fault of the engine's"
    );
    let held = |name: &str| found[name].as_ref().expect_err(name).clone();
    for name in ["named", "crlf"] {
        assert!(found[name].is_ok(), "{name}: {:?}", found[name].as_ref().err());
    }
    for name in ["longer", "prefix", "directory", "other", "none"] {
        let why = held(name);
        assert!(
            why.contains("[protected] paths must hold wf-agents.toml itself"),
            "{name}: {why}"
        );
    }
    assert!(held("empty").contains("wf-agents.toml: "), "{}", held("empty"));
    assert!(held("not_text").contains("not UTF-8"), "{}", held("not_text"));
}

// The declaration of one project says what its agents may not run, and no other project's does: two projects in one
// installation, each carried with its own heavy commands and its own text, as read from its main, which the owner's
// page shows; and the declaration of one that names a time no clock can count is not read at all, the project being
// held, with the others going on. Three git repositories: about 0.3 s alone.
#[test]
fn each_project_is_carried_with_its_own_heavy_commands_and_a_time_no_one_can_count_holds_only_its_project() {
    let dir = tempfile::tempdir().expect("a directory");
    let good = base();
    let heavy = "commands = [\"cargo\", \"rustc\", \"rustup\", \"npm\", \"npx\", \"node --test\", \"tsc\", \"vitest\", \"hyperfine\"]";
    assert!(good.contains(heavy), "the declaration names its heavy commands");
    let other = good.replace(heavy, "commands = [\"make\"]");
    let huge = good.replacen("too_long = \"2h\"", "too_long = \"213503982334602d\"", 1);
    assert_ne!(huge, good, "the fast workflow's time is written as the test expects");
    let projects = vec![
        ("with", good.clone().into_bytes()),
        ("other", other.clone().into_bytes()),
        ("huge", huge.into_bytes()),
    ];
    let (found, faults) = found(dir.path(), &projects);
    assert_eq!(faults, Vec::<String>::new());
    let carried = |name: &str| {
        found[name]
            .as_ref()
            .unwrap_or_else(|why| panic!("{name} is held: {why}"))
    };
    assert_eq!(carried("with").declaration.heavy.commands.len(), 9);
    assert_eq!(carried("other").declaration.heavy.commands, ["make"]);
    for (name, text) in [("with", &good), ("other", &other)] {
        assert_eq!(
            carried(name).text,
            DeclarationText {
                at: "refs/heads/main:wf-agents.toml".to_string(),
                text: text.clone(),
            },
            "{name}"
        );
    }
    let why = found["huge"]
        .as_ref()
        .expect_err("a time no one can count holds its project");
    assert!(why.contains("ci.fast.too_long"), "{why}");
}
