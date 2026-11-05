//! The engine's daily copy of the database kept off the machine as well: compressed, and uploaded as an asset of a
//! release of the data directory's own repository on GitHub, which is private, the newest kept there as beside the
//! database ([`db::backup`]). The repository is the data directory's git remote `origin`, so nothing is configured; a
//! data directory with no remote on GitHub keeps its copies on its machine only.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The release of the data directory's repository whose assets the copies are.
pub const RELEASE: &str = "database-backups";

/// The data directory's repository on GitHub, as owner/name, from its git remote `origin`; none when it has none
/// there, as a data directory cloned from a folder.
pub fn repository(data: &Path) -> Option<String> {
    let out = config::shared_git::command(data)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    github_name(String::from_utf8_lossy(&out.stdout).trim())
}

/// A GitHub remote's owner/name, from its address over SSH or HTTPS; none for any other address.
pub fn github_name(url: &str) -> Option<String> {
    let rest = ["git@github.com:", "ssh://git@github.com/", "https://github.com/"]
        .iter()
        .find_map(|p| url.strip_prefix(p))?;
    let name = rest.trim_end_matches('/').trim_end_matches(".git");
    match name.split('/').collect::<Vec<_>>()[..] {
        [owner, repo] if !owner.is_empty() && !repo.is_empty() => Some(format!("{owner}/{repo}")),
        _ => None,
    }
}

/// Compresses `copy` with gzip into a file beside it, its name with `.gz` after, and gives that file.
pub fn compress(copy: &Path) -> Result<PathBuf, String> {
    let to = PathBuf::from(format!("{}.gz", copy.display()));
    let failed = |e: std::io::Error| format!("{}: {e}", to.display());
    let mut from = BufReader::new(File::open(copy).map_err(|e| format!("{}: {e}", copy.display()))?);
    let out = BufWriter::new(File::create(&to).map_err(failed)?);
    let mut gz = flate2::write::GzEncoder::new(out, flate2::Compression::default());
    std::io::copy(&mut from, &mut gz).map_err(failed)?;
    gz.finish().map_err(failed)?.flush().map_err(failed)?;
    Ok(to)
}

/// Uploads `file` as an asset of [`RELEASE`] of `repo`, which is made when missing, and keeps the `keep` newest of
/// its assets of the same kind (those whose names start as `file`'s does, up to its time), deleting the older ones.
pub fn upload(repo: &str, file: &Path, keep: usize) -> Result<(), String> {
    let gh = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("gh").args(args).output().map_err(|e| format!("gh: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!(
                "gh {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    };
    if gh(&["release", "view", RELEASE, "--repo", repo, "--json", "tagName"]).is_err() {
        gh(&[
            "release",
            "create",
            RELEASE,
            "--repo",
            repo,
            "--title",
            "Database backups",
            "--notes",
            "Copies of the wf-agents database, compressed, the newest kept.",
            "--latest=false",
        ])?;
    }
    let path = file.to_string_lossy();
    gh(&["release", "upload", RELEASE, &path, "--repo", repo, "--clobber"])?;
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let kind = name.split('-').next().unwrap_or_default();
    let listed = gh(&[
        "release",
        "view",
        RELEASE,
        "--repo",
        repo,
        "--json",
        "assets",
        "--jq",
        ".assets[].name",
    ])?;
    let mut ours: Vec<&str> = listed
        .lines()
        .filter(|n| n.starts_with(&format!("{kind}-")) && n.ends_with(".sqlite.gz"))
        .collect();
    ours.sort_unstable();
    for old in &ours[..ours.len().saturating_sub(keep)] {
        gh(&["release", "delete-asset", RELEASE, old, "--repo", repo, "--yes"])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn a_remote_on_github_is_named_and_any_other_is_not() {
        for url in [
            "git@github.com:Ming-Research/wf-agents-data.git",
            "https://github.com/Ming-Research/wf-agents-data",
            "https://github.com/Ming-Research/wf-agents-data.git/",
            "ssh://git@github.com/Ming-Research/wf-agents-data.git",
        ] {
            assert_eq!(
                github_name(url).as_deref(),
                Some("Ming-Research/wf-agents-data"),
                "{url}"
            );
        }
        for url in [
            "/Users/me/wf-agents-data",
            "git@gitlab.com:o/n.git",
            "https://github.com/o",
            "",
        ] {
            assert_eq!(github_name(url), None, "{url}");
        }
    }

    #[test]
    fn a_compressed_copy_gives_back_the_copy() {
        let dir = std::env::temp_dir().join(format!("wf-agents-compress-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("daily-1.sqlite");
        let body: Vec<u8> = (0..100_000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
        std::fs::write(&copy, &body).unwrap();
        let gz = compress(&copy).unwrap();
        assert_eq!(gz, dir.join("daily-1.sqlite.gz"));
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(File::open(&gz).unwrap())
            .read_to_end(&mut back)
            .unwrap();
        assert_eq!(back, body);
        assert!(std::fs::metadata(&gz).unwrap().len() < body.len() as u64);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
