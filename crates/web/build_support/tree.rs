//! Reading a directory for a build script, and the Rust text that builds its files into a program. Shared by the build
//! script of `web`, which embeds the page, and that of `wf-agents`, which embeds the flows, through `#[path]`: a
//! third package for it would change the lock file, which every CI run holds fixed (`--locked`).

use std::path::{Path, PathBuf};

/// Every file under `root`, each by its path from `root` with `/` between the parts and where it is, in the byte order
/// of that path; or why the directory cannot be read. A name that begins with a dot is a tool's own, such as the
/// `.DS_Store` the Finder leaves, and is not part of what is built in.
pub fn read_tree(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = Vec::new();
    walk(root, "", &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, prefix: &str, found: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{prefix}{name}/"), found)?;
        } else {
            found.push((format!("{prefix}{name}"), path));
        }
    }
    Ok(())
}

/// The text of `pub static NAME`, a slice of each file's path and its bytes, which the program holds; the bytes are
/// read from where each file is, by its full path.
pub fn generate(files: &[(String, PathBuf)], name: &str) -> Result<String, String> {
    let mut text = format!("pub static {name}: &[(&str, &[u8])] = &[\n");
    for (relative, path) in files {
        let full = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
        text.push_str(&format!(
            "    ({relative:?}, include_bytes!({:?})),\n",
            full.display().to_string()
        ));
    }
    text.push_str("];\n");
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_of_a_directory_come_by_their_paths_in_byte_order_without_the_tools_own() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("b/c")).unwrap();
        std::fs::write(dir.path().join("b/c/z.txt"), "z").unwrap();
        std::fs::write(dir.path().join("b/a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("B.txt"), "B").unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join(".DS_Store"), "kept by the Finder").unwrap();
        std::fs::write(dir.path().join("b/.hidden"), "x").unwrap();
        let names: Vec<String> = read_tree(dir.path())
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["B.txt", "a.txt", "b/a.txt", "b/c/z.txt"]);
    }

    #[test]
    fn a_directory_that_cannot_be_read_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let error = read_tree(&dir.path().join("none")).unwrap_err();
        assert!(error.contains("none"), "{error}");
    }

    #[test]
    fn the_text_built_holds_each_file_by_its_relative_path_and_where_it_is() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        let files = read_tree(dir.path()).unwrap();
        let text = generate(&files, "THINGS").unwrap();
        let there = dir.path().join("a.txt").canonicalize().unwrap();
        assert_eq!(
            text,
            format!(
                "pub static THINGS: &[(&str, &[u8])] = &[\n    (\"a.txt\", include_bytes!({:?})),\n];\n",
                there.display().to_string()
            )
        );
    }
}
