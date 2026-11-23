//! The two lines every commit of a work ends with, which name its item and its work, so that a commit leads back to the
//! item that asked for it (`git log --grep='^Item: 36$'`). The agents' commits get them from a hook of the engine's,
//! handed to the git of each round; the engine's own commits and the merge into main get them from the engine's words
//! (`agents/commits.toml`).

use std::path::{Path, PathBuf};

use flow::agents::Commits;
use store::{ItemNumber, WorkNumber};

/// The directory of the data directory that holds the hook.
pub const HOOKS: &str = "hooks";

/// The hook's file name: git runs it as a commit's message is made, `-m` or an editor, `--no-verify` or not.
const HOOK: &str = "prepare-commit-msg";

/// The variable that gives the hook the line that names the item.
const ITEM_LINE: &str = "WF_ITEM_LINE";

/// The variable that gives the hook the line that names the work.
const WORK_LINE: &str = "WF_WORK_LINE";

/// What the hook is: it gives the message the two lines its variables hold, once, in place of any lines of that kind it
/// has already (a commit taken from another work ends with that work's), and nothing where it is given none.
const SCRIPT: &str = "#!/bin/sh\n\
# Written by the engine at its start, for the git of the agents' rounds: it ends each commit message with the\n\
# lines that name its item and its work.\n\
[ -n \"$WF_ITEM_LINE\" ] && [ -n \"$WF_WORK_LINE\" ] || exit 0\n\
exec git interpret-trailers --in-place --if-exists replace \\\n\
  --trailer \"$WF_ITEM_LINE\" --trailer \"$WF_WORK_LINE\" \"$1\"\n";

/// The item and the work whose commits these are, by the numbers the two lines name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Whose {
    pub item: ItemNumber,
    pub work: WorkNumber,
}

impl Whose {
    /// The message with the two lines at its end.
    pub fn ending(&self, commits: &Commits, message: &str) -> String {
        commits.ending(message, self.item.0, self.work.0)
    }
}

/// The directory the hook is in, in the data directory.
pub fn directory(data: &Path) -> PathBuf {
    data.join(HOOKS)
}

/// Writes the hook in the data directory, where it is not already as it should be: the engine does it as it starts, and
/// again at the next start, so that a hook of an older engine is replaced.
pub fn install(data: &Path) -> Result<(), String> {
    let dir = directory(data);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(HOOK);
    if std::fs::read_to_string(&file).is_ok_and(|text| text == SCRIPT) && executable(&file) {
        return Ok(());
    }
    // Written beside the file and moved onto it, so that a commit made as the engine starts finds a whole hook.
    let writing = dir.join(format!("{HOOK}.part"));
    std::fs::write(&writing, SCRIPT).map_err(|e| format!("{}: {e}", writing.display()))?;
    make_executable(&writing)?;
    std::fs::rename(&writing, &file).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(unix)]
fn executable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(file).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    true
}

#[cfg(unix)]
fn make_executable(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(file, mode).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), String> {
    Ok(())
}

/// What a round's command line is given beside the engine's environment, so that the git of its agents runs the hook
/// for this work: the hook's directory as git's `core.hooksPath`, by git's own variables, which change no configuration
/// of the repository, and the two lines.
pub fn environment(data: &Path, commits: &Commits, whose: Whose) -> Vec<(String, String)> {
    let (item, work) = commits.lines(whose.item.0, whose.work.0);
    vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        ("GIT_CONFIG_KEY_0".to_string(), "core.hooksPath".to_string()),
        (
            "GIT_CONFIG_VALUE_0".to_string(),
            directory(data).to_string_lossy().into_owned(),
        ),
        (ITEM_LINE.to_string(), item),
        (WORK_LINE.to_string(), work),
    ]
}

/// The message of the squash that merges a work, from the messages of its commits, oldest first: one commit gives its
/// message, several give each its first line after a star and then the rest, the lines that name an item or a work left
/// out, whichever item and work they name, and the commits' `Co-authored-by` lines, each once, go to the end, where the
/// host reads them, before the work's own two lines. `tidy` is given each piece the commits' agents wrote, to make it
/// fit to publish.
pub fn squashed(commits: &Commits, whose: Whose, messages: &[String], tidy: &dyn Fn(&str) -> String) -> String {
    let (item_line, work_line) = commits.lines(whose.item.0, whose.work.0);
    let mut co_authors: Vec<String> = Vec::new();
    let mut parts: Vec<(String, String)> = Vec::new();
    for message in messages {
        let mut subject = None;
        let mut rest: Vec<&str> = Vec::new();
        for line in message.lines() {
            let said = line.trim_end();
            if commits.is_one_of_the_lines(said) {
                continue;
            }
            if let Some(author) = co_author(said, tidy) {
                if !co_authors.iter().any(|seen| seen.eq_ignore_ascii_case(&author)) {
                    co_authors.push(author);
                }
                continue;
            }
            if subject.is_none() {
                subject = Some(said);
            } else {
                rest.push(said);
            }
        }
        let body = rest.join("\n");
        parts.push((subject.unwrap_or_default().to_string(), body.trim().to_string()));
    }
    let text = match parts.as_slice() {
        [] => String::new(),
        [(_, body)] => tidy(body),
        many => many
            .iter()
            .map(|(subject, body)| {
                if body.is_empty() {
                    format!("* {}", tidy(subject))
                } else {
                    format!("* {}\n\n{}", tidy(subject), tidy(body))
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    };
    let mut ending: Vec<String> = co_authors;
    ending.push(item_line);
    ending.push(work_line);
    let ending = ending.join("\n");
    if text.trim().is_empty() {
        ending
    } else {
        format!("{}\n\n{ending}", text.trim())
    }
}

/// The line as the host reads a co-author, `Co-authored-by: Name <email>`, where it is exactly that: the name made fit
/// to publish by `tidy` and the email, which a host reads by its `@`, as it is. A line with anything else after the
/// email, or an email that could name a mention or a reference, is no co-author and is left to be said as any other
/// text of the agents.
fn co_author(line: &str, tidy: &dyn Fn(&str) -> String) -> Option<String> {
    let (key, value) = line.split_once(':')?;
    if !key.trim().eq_ignore_ascii_case("co-authored-by") {
        return None;
    }
    let (name, email) = value.trim().strip_suffix('>')?.split_once('<')?;
    let name = name.trim();
    let address = email
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty() && !domain.contains('@'));
    let plain = !email.contains(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '#'));
    if name.is_empty() || name.contains(['<', '>']) || !address || !plain {
        return None;
    }
    let name = tidy(name);
    // A name cut short by `tidy` has a line of its own after it, which no co-author line has.
    (!name.contains('\n')).then(|| format!("Co-authored-by: {name} <{email}>"))
}

#[cfg(test)]
mod tests;
