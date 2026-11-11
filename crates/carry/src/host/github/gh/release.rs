//! The copies of the database kept off the machine, as assets of a release of a private repository on GitHub, asked
//! with the same runner of `gh` as the host: the environment cleared, the token only as `GH_TOKEN`, a configuration
//! directory of its own, a call that does not end stopped and killed, and nothing secret in a failure's words. The token
//! it is given is the backup's own, not the host's.

use std::path::Path;
use std::time::Duration;

use backup::{Offsite, RELEASE};

use super::Gh;

/// The lowest rate, in bytes a second, that an upload of a copy is expected to go at. The limit of an upload is the
/// runner's own limit, which fits an answer from GitHub, and the time the copy takes at this rate.
///
/// This is a judgment, not a measurement: nothing that implemented it could upload to GitHub. It is set low on purpose,
/// 64 KiB a second or about half a megabit, because the limit exists to end a call that hangs and not to judge the
/// owner's line. Too tight, a copy on a slow line would be killed on every hourly try, never be uploaded, and waste the
/// bandwidth each try used; too loose, a call that hangs costs only a later try. At this rate a copy of 200 MB is bounded
/// under an hour. It is no setting: there is no value the owner needs to tune under so loose a bound.
pub const UPLOAD_RATE: u64 = 64 * 1024;

/// A release of a private repository on GitHub, reached with `gh` and the token it is given.
pub struct GhOffsite {
    gh: Gh,
}

impl GhOffsite {
    /// The release of the repository `owner/name`, reached with this command line and the backup's own token, keeping
    /// the command's configuration in this directory.
    pub fn new(command: &str, repository: &str, token: &str, config_dir: &Path) -> GhOffsite {
        GhOffsite {
            gh: Gh::new(command, repository, token, config_dir),
        }
    }

    /// The same, giving any call up after this long, the floor of an upload's limit.
    #[cfg(test)]
    pub(crate) fn within(self, limit: Duration) -> GhOffsite {
        GhOffsite {
            gh: self.gh.within(limit),
        }
    }

    /// How long an upload of a file of this many bytes may take before it is given up: the runner's limit, then the time
    /// the file takes at the lowest rate.
    pub(crate) fn upload_limit(floor: Duration, bytes: u64) -> Duration {
        floor + Duration::from_secs(bytes / UPLOAD_RATE)
    }

    fn repository(&self) -> &str {
        &self.gh.repository
    }

    /// Makes the release where it is not there, which is not the latest and holds the copies as assets. Where it can be
    /// neither seen nor made, that is the error.
    async fn ensure_release(&self) -> Result<(), String> {
        let view = [
            "release",
            "view",
            RELEASE,
            "--repo",
            self.repository(),
            "--json",
            "tagName",
        ];
        if self.gh.output(&view, self.gh.limit).await.is_ok() {
            return Ok(());
        }
        let create = [
            "release",
            "create",
            RELEASE,
            "--repo",
            self.repository(),
            "--title",
            "Database backups",
            "--notes",
            "Copies of the wf-agents database, compressed, the newest kept.",
            "--latest=false",
        ];
        self.gh.output(&create, self.gh.limit).await.map(|_| ())
    }
}

#[cfg(test)]
mod tests;

impl Offsite for GhOffsite {
    async fn private(&self) -> Result<bool, String> {
        let path = format!("repos/{}", self.repository());
        let found = self.gh.json("GET", &path, None, &[200]).await?;
        found["private"]
            .as_bool()
            .ok_or_else(|| "GitHub's answer about the repository does not say whether it is private".to_string())
    }

    async fn upload(&self, file: &Path) -> Result<(), String> {
        let size = (std::fs::metadata(file).map_err(|e| format!("{}: {e}", file.display()))?).len();
        self.ensure_release().await?;
        let file = file.to_string_lossy().into_owned();
        let args = [
            "release",
            "upload",
            RELEASE,
            file.as_str(),
            "--repo",
            self.repository(),
            "--clobber",
        ];
        let limit = GhOffsite::upload_limit(self.gh.limit, size);
        self.gh.output(&args, limit).await.map(|_| ())
    }

    async fn list(&self) -> Result<Vec<String>, String> {
        // The release is there before it is listed, so that a place with none is an empty one, and a listing that
        // fails for any reason is an error and never looks like an empty place.
        self.ensure_release().await?;
        let args = [
            "release",
            "view",
            RELEASE,
            "--repo",
            self.repository(),
            "--json",
            "assets",
            "--jq",
            ".assets[].name",
        ];
        let listed = self.gh.output(&args, self.gh.limit).await?;
        Ok(listed.lines().map(str::to_string).collect())
    }

    async fn remove(&self, name: &str) -> Result<(), String> {
        let args = [
            "release",
            "delete-asset",
            RELEASE,
            name,
            "--repo",
            self.repository(),
            "--yes",
        ];
        self.gh.output(&args, self.gh.limit).await.map(|_| ())
    }
}
