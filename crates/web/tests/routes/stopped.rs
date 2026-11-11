//! The engine stopped by its store as the owner acts: the refusal says whether what was asked was kept.

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::json;
use store::{Item, ItemNumber, Storage, Store, WorkHead, WorkNumber};
use work::{Time, Work};

use crate::{post, running_on};

/// A store on a file whose saves fail once `failing` is set.
struct Failing {
    store: Store,
    failing: Arc<Mutex<bool>>,
}

impl Storage for Failing {
    fn add(
        &mut self,
        title: &str,
        level: &str,
        depends_on: &[ItemNumber],
        flow: &str,
        at: Time,
        first: &Work,
    ) -> Result<(ItemNumber, WorkNumber), store::Fault> {
        self.store.add(title, level, depends_on, flow, at, first)
    }

    fn set_level(&mut self, item: ItemNumber, level: &str, at: Time) -> Result<(), store::Fault> {
        self.store.set_level(item, level, at)
    }

    fn items(&self) -> Result<Vec<Item>, store::Fault> {
        self.store.items()
    }

    fn works(&self) -> Result<Vec<WorkHead>, store::Fault> {
        self.store.works()
    }

    fn load(&self, work: WorkNumber) -> Result<Work, store::Fault> {
        self.store.load(work)
    }

    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), store::Fault> {
        if *self.failing.lock().unwrap() {
            return Err(store::Fault::Database("the disk is full".to_string()));
        }
        self.store.save(work, given)
    }
}

#[tokio::test(start_paused = true)]
async fn an_act_the_engine_stopped_before_keeping_says_it_was_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    let failing = Arc::new(Mutex::new(false));
    let store = Failing {
        store: Store::open(dir.path().join("store.sqlite")).unwrap(),
        failing: failing.clone(),
    };
    let running = running_on(Box::new(store), dir);
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    });
    assert_eq!(post(&running.router, "/api/v1/items", &item).await.0, StatusCode::OK);
    *failing.lock().unwrap() = true;
    assert_eq!(
        post(&running.router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "error": "The engine stopped: the store failed: the database failed: the disk is full. What you asked \
                          was not kept: give it again once the engine runs.",
                "kept": false,
            })
        )
    );
    // The loop has stopped.
    let (status, words) = crate::refused(&running.router, "/api/v1/items").await;
    assert_eq!(
        (status, words.as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, "The engine is not running.")
    );
}
