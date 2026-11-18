//! What a work changed and the pull request it is checked through: its summary's `changed` and `pull`, and
//! `GET /api/v1/works/{work}/changes` and `.../changes/diff`, as the carrier of its project's repository says them off
//! the loop.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use service::{Changes, Diff, Diffstat, FileChanged, OfWork, Pull, Round, RoundReply, Rounds, Scripted, Seen};
use store::Store;

use crate::{Running, get_json, greeted, pass, post, refused, running, running_carried, trial_script};

/// The trial's acts, whose repository says each work changed two files, ten lines added and three removed, gives the
/// changes of work 1 and the diff of its `src/a.rs`, and names the pull request numbered 5 for each work it is told
/// merged.
struct Repository {
    acts: Arc<dyn Rounds>,
}

impl Rounds for Repository {
    fn give(&self, round: Round, reply: RoundReply) {
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn seen(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        Box::pin(async move {
            let changed = Diffstat {
                files: 2,
                added: 10,
                removed: 3,
            };
            let pull = of.merged.map(|_| Pull {
                number: 5,
                url: "https://github.com/o/r/pull/5".to_string(),
            });
            Ok(Seen {
                changed: Some(changed),
                pull,
            })
        })
    }

    fn changes(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        Box::pin(async move {
            let file = |path: &str, added, removed, binary, committed| FileChanged {
                path: path.to_string(),
                added,
                removed,
                binary,
                committed,
            };
            Ok((of.work.0 == 1).then(|| Changes {
                base: "b0".to_string(),
                head: "h1".to_string(),
                worktree: true,
                files: vec![
                    file("logo.png", 0, 0, true, false),
                    file("src/a.rs", 10, 3, false, true),
                ],
            }))
        })
    }

    fn diff(
        &self,
        of: OfWork,
        path: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Diff>, String>> + Send + '_>> {
        Box::pin(async move {
            Ok((of.work.0 == 1 && path == "src/a.rs").then(|| Diff {
                path,
                text: "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new".to_string(),
                cut: None,
            }))
        })
    }
}

/// The script's agents, whose first round says, as the host would as it checks the work, the pull request it is
/// checked through.
struct Pulling {
    agents: Arc<dyn Rounds>,
}

impl Rounds for Pulling {
    fn give(&self, round: Round, reply: RoundReply) {
        if (round.work.0, round.run.0, round.number) == (1, 1, 1) {
            reply.hosting().pulled(7, "https://github.com/o/r/pull/7".to_string());
        }
        self.agents.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.agents.stop(round, reply);
    }
}

/// The trial flow run with its script, its acts' repository as [`Repository`] says it, and its agents' first round
/// saying its pull request where `pulling`.
fn with_repository(pulling: bool) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    carriers.acts = Arc::new(Repository {
        acts: carriers.acts.clone(),
    });
    if pulling {
        carriers.agents = Arc::new(Pulling {
            agents: carriers.agents.clone(),
        });
    }
    running_carried(Box::new(store), dir, carriers)
}

#[tokio::test(start_paused = true)]
async fn a_works_summary_says_what_it_changed_and_the_pull_request_it_keeps() {
    let running = with_repository(true);
    let router = &running.router;
    greeted(router).await;
    let summary = get_json(router, "/api/v1/works/1").await["summary"].clone();
    let changed = json!({ "files": 2, "added": 10, "removed": 3 });
    let pull = json!({ "number": 7, "url": "https://github.com/o/r/pull/7" });
    assert_eq!((&summary["changed"], &summary["pull"]), (&changed, &pull));
    let listed = get_json(router, "/api/v1/items").await["items"][0]["work"].clone();
    assert_eq!(listed, summary, "the list gives the same summary");
}

// A work that merged before works kept their pull requests is given the one the commit its merge made names, which the
// repository's carrier reads from what the merge wrote; a work that has not merged is asked for none.
#[tokio::test(start_paused = true)]
async fn a_work_that_merged_keeping_no_pull_request_is_given_the_one_its_merge_names() {
    let running = with_repository(false);
    let router = &running.router;
    greeted(router).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["pull"],
        Value::Null
    );
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    let mut summary = Value::Null;
    for _ in 0..240 {
        summary = get_json(router, "/api/v1/works/1").await["summary"].clone();
        if summary["now"]["kind"] == "merged" {
            break;
        }
        pass(250).await;
    }
    assert_eq!(summary["now"]["kind"], "merged", "{summary}");
    let pull = json!({ "number": 5, "url": "https://github.com/o/r/pull/5" });
    assert_eq!(summary["pull"], pull);
}

#[tokio::test(start_paused = true)]
async fn each_file_a_work_changed_and_the_diff_of_one_are_given_and_what_there_is_none_of_refused() {
    let running = with_repository(false);
    let router = &running.router;
    greeted(router).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1/changes").await,
        json!({
            "base": "b0",
            "head": "h1",
            "worktree": true,
            "files": [
                { "path": "logo.png", "added": 0, "removed": 0, "binary": true, "committed": false },
                { "path": "src/a.rs", "added": 10, "removed": 3, "binary": false, "committed": true },
            ],
        })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/changes/diff?path=src%2Fa.rs").await,
        json!({
            "path": "src/a.rs",
            "diff": "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new",
            "cut": null,
        })
    );
    let none = StatusCode::NOT_FOUND;
    assert_eq!(
        refused(router, "/api/v1/works/1/changes/diff?path=src%2Fb.rs").await,
        (none, "Work 1 changed no file src/b.rs.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/9/changes").await,
        (none, "There is no work 9.".to_string())
    );
    let (status, words) = refused(router, "/api/v1/works/1/changes/diff").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(words.starts_with("The file cannot be read: "), "{words}");
    // A carrier that keeps no repository has nothing to tell of a work.
    let scripted = running();
    greeted(&scripted.router).await;
    assert_eq!(
        refused(&scripted.router, "/api/v1/works/1/changes").await,
        (
            none,
            "Work 1 has no change to tell yet: its branch is not made.".to_string()
        )
    );
    let summary = get_json(&scripted.router, "/api/v1/works/1").await["summary"].clone();
    assert_eq!((&summary["changed"], &summary["pull"]), (&Value::Null, &Value::Null));
}
