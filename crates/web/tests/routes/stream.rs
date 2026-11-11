//! `GET /api/v1/changes`: the stream of changes, as server-sent events.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use crate::{post, running};

/// The next event of the stream, past the comments that keep it open.
async fn next(body: &mut Body) -> String {
    loop {
        let frame = body.frame().await.unwrap().unwrap();
        let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
        if !text.starts_with(':') {
            return text;
        }
    }
}

#[tokio::test(start_paused = true)]
async fn the_stream_says_as_it_opens_that_everything_may_have_changed_and_then_each_change() {
    let running = running();
    let router = &running.router;
    let item = |title: &str| {
        json!({
            "title": title,
            "dependsOn": [],
            "flow": "trial",
            "writes": [{ "block": "request", "content": "Please.", "value": null }],
            "held": true,
        })
    };
    assert_eq!(post(router, "/api/v1/items", &item("First")).await.0, StatusCode::OK);

    let request = Request::builder().uri("/api/v1/changes").body(Body::empty()).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "text/event-stream");
    let mut body = response.into_body();
    assert_eq!(next(&mut body).await, "data: {\"works\":[1],\"items\":true}\n\n");

    // A pause of a work added paused is one more pause, and so changes it.
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "pause" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(next(&mut body).await, "data: {\"works\":[1],\"items\":false}\n\n");
    assert_eq!(post(router, "/api/v1/items", &item("Second")).await.0, StatusCode::OK);
    assert_eq!(next(&mut body).await, "data: {\"works\":[2],\"items\":true}\n\n");
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(next(&mut body).await, "data: {\"works\":[1],\"items\":false}\n\n");
    // The plan's round begins when the work is let on, and ends a second and a half later.
    assert_eq!(next(&mut body).await, "data: {\"works\":[1],\"items\":false}\n\n");
}
