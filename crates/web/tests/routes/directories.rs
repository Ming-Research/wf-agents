//! The small flow kept in `tests/flows/`, which the page is shown with: read from its directory as the command reads
//! it, so that it stays a definition that compiles.

use std::path::PathBuf;

use crate::{get_json, has};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/flows").join(name)
}

#[tokio::test]
async fn the_small_flow_compiles() {
    let router = web::router(web::Flows::dirs(dir("small"), Box::new(has)));
    let flow = get_json(&router, "/api/v1/flow").await;
    assert_eq!(flow["state"], "compiled", "{flow}");
}
