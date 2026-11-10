//! The page itself, served from the files built into the binary.

use axum::http::{Method, StatusCode};

use crate::{Shared, ask, serve, tiny};

#[tokio::test]
async fn the_root_and_any_other_path_of_the_page_are_its_index() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    for path in ["/", "/cells/plan"] {
        let (status, content_type, body) = ask(&router, Method::GET, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(content_type, "text/html; charset=utf-8", "{path}");
        assert!(String::from_utf8_lossy(&body).contains("<div id=\"root\">"), "{path}");
    }
}

#[tokio::test]
async fn a_missing_asset_a_path_of_the_interface_that_is_none_and_a_write_are_refused() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    let (status, ..) = ask(&router, Method::GET, "/assets/none.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, content_type, _) = ask(&router, Method::GET, "/api/v1/nothing").await;
    assert_eq!(
        (status, content_type.as_str()),
        (StatusCode::NOT_FOUND, "application/json")
    );
    let (status, ..) = ask(&router, Method::POST, "/").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
