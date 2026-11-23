//! Where a child's program is found, against directories made for each test, and the commands the engine starts by
//! the path found. Each test reads a few files and builds a command, starting nothing, in well under a second.

use super::*;

/// Makes a file of this name in the directory, which may be run or not.
fn file(dir: &Path, name: &str, runnable: bool) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\n").unwrap();
    let mode = if runnable { 0o755 } else { 0o644 };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

#[test]
fn a_program_is_found_in_the_first_directory_that_holds_it_as_a_file_that_may_be_run() {
    let dirs = tempfile::tempdir().unwrap();
    let [first, second, third] = ["first", "second", "third"].map(|name| {
        let dir = dirs.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        dir
    });
    file(&first, "tool", false);
    std::fs::create_dir(second.join("other")).unwrap();
    let found = file(&third, "tool", true);
    file(&third, "other", true);
    let path = [&first, &second, &third].map(|dir| dir.to_str().unwrap()).join(":");
    assert_eq!(
        find("tool", Some(&path)),
        Some(found.clone()),
        "a file that may not be run is passed over"
    );
    assert_eq!(
        find("other", Some(&path)),
        Some(third.join("other")),
        "a directory of the name is no program"
    );
    assert_eq!(find("missing", Some(&path)), None);
    assert_eq!(find("tool", None), None, "no PATH");
    assert_eq!(
        find(&found.to_string_lossy(), Some(&path)),
        None,
        "a path is started as it is"
    );
    assert_eq!(find("", Some(&path)), None);
    // A directory read against the child's own directory comes first: the system looks the name up.
    for before in ["", "relative"] {
        let path = format!("{}:{before}:{}", first.display(), third.display());
        assert_eq!(find("tool", Some(&path)), None, "{path}");
    }
}

#[test]
fn a_program_on_the_engines_path_is_started_by_its_path_and_said_by_its_name() {
    let started = command("sh");
    let program = Path::new(started.as_std().get_program());
    assert!(program.is_absolute(), "{}", program.display());
    assert_eq!(program.file_name(), Some(OsStr::new("sh")));
    assert!(runnable(program));
    assert_eq!(path("sh"), program, "found once and kept");
    assert_eq!(named(program.as_os_str()), "sh");
    // A name no directory holds is left to the system, and said as it is.
    let missing = "no-such-program-of-wf-agents";
    assert_eq!(path(missing), Path::new(missing));
    assert_eq!(named(OsStr::new(missing)), missing);
    assert_eq!(named(OsStr::new("/bin/no-such-program")), "/bin/no-such-program");
}

#[test]
fn a_child_is_given_of_the_engines_environment_only_what_is_kept() {
    let names: Vec<&str> = kept().iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        (names.iter()).all(|name| cli::KEPT.contains(name) || name.starts_with("LC_")),
        "{names:?}"
    );
    let path = engine().iter().find(|(name, _)| name == "PATH");
    assert_eq!(kept().iter().find(|(name, _)| name == "PATH"), path);
}
