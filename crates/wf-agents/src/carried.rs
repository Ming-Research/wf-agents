//! The flows the program carries: the files of the repository's `flow/` as they were when the program was built
//! (`build.rs`), which the engine takes out into its data directory as it starts, so that it runs the flows of its own
//! source and never those of a later main that it may not read.

use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/flows.rs"));

/// What a flow's directory is called inside the data directory's `flows/`, which holds the carried files whole: each
/// flow beside the files that every flow shares.
pub const TAKEN_INTO: &str = "flows";

/// The names of the flows `files` carry: each of their directories that holds a flow's file, in order.
pub fn names<'a>(files: &[(&'a str, &[u8])]) -> Vec<&'a str> {
    let mut names: Vec<&str> = files
        .iter()
        .filter_map(|(path, _)| path.strip_suffix(flow::FLOW_FILE))
        .filter_map(|dir| dir.strip_suffix('/'))
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Takes the files out into `data`'s `flows/`, in place of what that holds, and gives the directory of each flow named,
/// in the order named; or says why not. A name that is no flow the files carry is refused before anything is changed,
/// with the names they carry.
///
/// The new files are written beside, and put in place by renaming, so that a stop in the middle leaves the old
/// directory or the new one whole; what a stopped start left beside is removed first.
pub fn take(data: &Path, files: &[(&str, &[u8])], wanted: &[String]) -> Result<Vec<PathBuf>, String> {
    let carried = names(files);
    let unknown: Vec<&str> = wanted
        .iter()
        .map(String::as_str)
        .filter(|name| !carried.contains(name))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "no flow {} is carried by this program, which carries {}",
            unknown.join(", "),
            carried.join(", ")
        ));
    }
    let (into, new, old) = (data.join(TAKEN_INTO), data.join("flows.new"), data.join("flows.old"));
    for leftover in [&new, &old] {
        remove(leftover)?;
    }
    for (path, bytes) in files {
        let place = new.join(path);
        if let Some(parent) = place.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&place, bytes).map_err(|e| format!("{}: {e}", place.display()))?;
    }
    if into.exists() {
        std::fs::rename(&into, &old).map_err(|e| format!("{}: {e}", into.display()))?;
    }
    std::fs::rename(&new, &into).map_err(|e| format!("{}: {e}", new.display()))?;
    remove(&old)?;
    Ok(wanted.iter().map(|name| into.join(name)).collect())
}

fn remove(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file under `dir` as its path from `root` and its bytes, found without the build script's code.
    fn on_disk(root: &Path, dir: &Path, found: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("the directory is read") {
            let path = entry.expect("an entry").path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if path.is_dir() {
                on_disk(root, &path, found);
            } else {
                let name = path.strip_prefix(root).expect("under the root");
                let name = name.to_string_lossy().replace('\\', "/");
                found.push((name, std::fs::read(&path).expect("the file is read")));
            }
        }
    }

    #[test]
    fn the_flows_built_in_are_the_flow_directory() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let mut found = Vec::new();
        on_disk(&root, &root, &mut found);
        found.sort();
        let mut carried: Vec<(String, Vec<u8>)> = FLOWS
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
            .collect();
        carried.sort();
        assert!(!carried.is_empty(), "no file of the flows is built in");
        assert_eq!(
            carried.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            found.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "the files built in are not the files of flow/"
        );
        for ((name, built), (_, disk)) in carried.iter().zip(&found) {
            assert!(built == disk, "{name} built in is not as flow/ has it");
        }
        assert!(
            carried.iter().any(|(name, _)| name == flow::agents::RULES_FILE),
            "what the engine says to every agent, beside the flows, is carried too, which the engine reads there"
        );
    }

    #[test]
    fn the_flows_the_program_carries_are_each_directory_that_holds_a_flows_file() {
        let files: [(&str, &[u8]); 5] = [
            ("beta/flow.toml", b""),
            ("alpha/flow.toml", b""),
            ("alpha/cells/one.toml", b""),
            ("shared/rules.toml", b""),
            ("alpha/nested/flow.toml", b""),
        ];
        assert_eq!(names(&files), ["alpha", "beta"]);
        // And the repository's own: the files built in name at least one.
        assert!(!names(FLOWS).is_empty());
    }

    fn files() -> [(&'static str, &'static [u8]); 3] {
        [
            ("alpha/flow.toml", b"alpha"),
            ("alpha/cells/one.toml", b"one"),
            ("agents/session.toml", b"shared"),
        ]
    }

    #[test]
    fn taking_the_flows_out_replaces_what_was_there_and_gives_each_named_directory() {
        let data = tempfile::tempdir().unwrap();
        let into = data.path().join("flows");
        std::fs::create_dir_all(into.join("alpha")).unwrap();
        std::fs::write(into.join("alpha/left-by-an-earlier-start.toml"), "old").unwrap();
        std::fs::create_dir_all(data.path().join("flows.new")).unwrap();
        std::fs::write(data.path().join("flows.new/half.toml"), "half").unwrap();
        let dirs = take(data.path(), &files(), &["alpha".to_string()]).unwrap();
        assert_eq!(dirs, [into.join("alpha")]);
        // The flows come in the order they are named.
        let two: [(&str, &[u8]); 2] = [("alpha/flow.toml", b""), ("beta/flow.toml", b"")];
        let named = ["beta".to_string(), "alpha".to_string()];
        assert_eq!(
            take(data.path(), &two, &named).unwrap(),
            [into.join("beta"), into.join("alpha")]
        );
        take(data.path(), &files(), &["alpha".to_string()]).unwrap();
        let mut found = Vec::new();
        on_disk(&into, &into, &mut found);
        found.sort();
        let wanted: Vec<(String, Vec<u8>)> = vec![
            ("agents/session.toml".to_string(), b"shared".to_vec()),
            ("alpha/cells/one.toml".to_string(), b"one".to_vec()),
            ("alpha/flow.toml".to_string(), b"alpha".to_vec()),
        ];
        assert_eq!(found, wanted);
        assert!(!data.path().join("flows.new").exists());
        assert!(!data.path().join("flows.old").exists());
    }

    #[test]
    fn a_flow_that_is_not_carried_is_refused_before_anything_changes() {
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(data.path().join("flows")).unwrap();
        std::fs::write(data.path().join("flows/kept.toml"), "kept").unwrap();
        let said = take(data.path(), &files(), &["alpha".to_string(), "gamma".to_string()]).unwrap_err();
        assert_eq!(said, "no flow gamma is carried by this program, which carries alpha");
        assert_eq!(
            std::fs::read_to_string(data.path().join("flows/kept.toml")).unwrap(),
            "kept"
        );
    }
}
