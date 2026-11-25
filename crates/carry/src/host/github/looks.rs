//! The looks at the runs of the project's workflows, which every wait for a run of a workflow shares: one look at a
//! workflow's runs at a time, each sent with the entity tag of the last answer, so that GitHub answers 304, which costs
//! nothing against its limit on requests, where nothing changed since; and an answer younger than a wait asks for is
//! given to it as it is. What waiting costs GitHub's limit grows with how often the runs of the workflows waited on
//! change, and not with how many waits there are or how long they last.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

use super::api::{Api, Listed, Run};
use crate::host::Failed;

/// The last answer to a look at the runs of each workflow on each branch.
#[derive(Default)]
pub(super) struct Looks {
    of: Mutex<HashMap<(String, String), Shared>>,
}

/// What the last look at the runs of a workflow on a branch found, locked while a wait looks at them, so that one look
/// at a time is under way.
type Shared = Arc<tokio::sync::Mutex<Seen>>;

/// What the last look at the runs of a workflow on a branch found.
#[derive(Default)]
struct Seen {
    runs: Vec<Run>,
    /// The entity tag of the answer that gave the runs, which the next look sends.
    tag: Option<String>,
    /// When the last look was sent; none before the first.
    at: Option<Instant>,
}

/// The runs of a workflow as a look found them, and when that look was sent.
pub(super) struct Found {
    pub(super) runs: Vec<Run>,
    pub(super) at: Instant,
}

impl Looks {
    /// The runs by hand of the workflow on the branch: as the last look found them, where it was sent less than
    /// `within` ago, so that a zero `within` asks for a look sent now; or else as a look sent now finds them. A wait
    /// that comes while another's look is under way is given that look's answer once it comes, where it is young
    /// enough.
    pub(super) async fn runs(
        &self,
        api: &impl Api,
        workflow: &str,
        branch: &str,
        within: Duration,
    ) -> Result<Found, Failed> {
        let seen = {
            let mut of = self.of.lock().unwrap_or_else(PoisonError::into_inner);
            let key = (workflow.to_string(), branch.to_string());
            of.entry(key).or_default().clone()
        };
        let mut seen = seen.lock().await;
        if let Some(at) = seen.at
            && at.elapsed() < within
        {
            return Ok(Found {
                runs: seen.runs.clone(),
                at,
            });
        }
        let sent = Instant::now();
        match api.runs(workflow, branch, seen.tag.as_deref()).await? {
            Listed::Runs { runs, tag } => {
                seen.runs = runs;
                seen.tag = tag;
            }
            Listed::Unchanged => {}
        }
        seen.at = Some(sent);
        Ok(Found {
            runs: seen.runs.clone(),
            at: sent,
        })
    }
}
