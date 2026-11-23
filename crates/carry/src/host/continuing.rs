//! What a host does to find where a work that continues a branch starts: the heads the local repository and the host
//! hold of the branch, and which of them the work starts from.

use std::path::Path;

use super::Failed;
use crate::git;

/// The head of the branch of this name in the local repository; none where it has no such branch.
pub(super) async fn local_head(repo: &Path, name: &str) -> Result<Option<String>, String> {
    let spec = format!("refs/heads/{name}^{{commit}}");
    let found = git::said(repo, &["rev-parse", "--verify", "--quiet", "--end-of-options", &spec]).await?;
    Ok((found.ok && !found.out.is_empty()).then_some(found.out))
}

/// Whether the commit is the other, or under it.
async fn under(repo: &Path, commit: &str, of: &str) -> Result<bool, String> {
    let said = git::said(repo, &["merge-base", "--is-ancestor", commit, of]).await?;
    match said.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor {commit} {of} failed: {}",
            said.err
        )),
    }
}

/// Which head of one branch a work starts from where the local repository and the host hold it at different commits:
/// the same one, or the one that holds the other. The engine pushes a work's branch only to have it checked, so the
/// local repository may hold commits the host does not. Two heads that have gone different ways are a refusal naming
/// both, since neither is the branch's.
pub(super) async fn newer(repo: &Path, name: &str, local: &str, host: &str, hosted: &str) -> Result<String, Failed> {
    if local == host || under(repo, host, local).await? {
        return Ok(local.to_string());
    }
    if under(repo, local, host).await? {
        return Ok(host.to_string());
    }
    Err(Failed::refused(format!(
        "the branch {name} has gone two ways: the local repository holds it at {local} and {hosted} at {host}, and \
         neither holds the other. Bring the two into one, or have the item continue another branch or none"
    )))
}
