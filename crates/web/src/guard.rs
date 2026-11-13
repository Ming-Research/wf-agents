//! The refusal of what could come from a page of another site, which every request passes through before any route.
//!
//! The engine serves its owner on the loopback address with no sign-in, so a request that reaches it acts as the owner:
//! it can let a work on, write a verdict that merges it, or add items that spend money. A page of any site the owner
//! opens can make the owner's browser send one: a form or a fetch posted to the loopback address is sent, only its
//! answer is kept from the page; and a name of that site resolved to the loopback address once its page has loaded
//! reaches the engine as that site's own. So a request is refused when its Host is not the engine's own address with
//! its port, when it has an Origin that is not the engine's own, when it asks the interface and the browser says
//! another site made it, and when it is a POST that is not JSON: a page of another site cannot send JSON without the
//! browser first asking the engine's leave, which the engine never gives.
//!
//! Brought over from the old engine's server (`crates/web/src/lib.rs` on `old-main`, from commit `078fe280`).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::fail;

/// The engine's own address on the port it listens on, as a Host and as an Origin, by both of its names.
pub(crate) struct Own {
    hosts: [String; 2],
    origins: [String; 2],
}

impl Own {
    pub(crate) fn at(port: u16) -> Arc<Own> {
        let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
        let origins = hosts.clone().map(|host| format!("http://{host}"));
        Arc::new(Own { hosts, origins })
    }
}

/// Lets through only what the engine's own page, the page's development server and the engine's command line send.
pub(crate) async fn guard(State(own): State<Arc<Own>>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let one_of = |value: Option<&str>, allowed: &[String]| {
        value.is_some_and(|v| allowed.iter().any(|a| a.eq_ignore_ascii_case(v)))
    };
    if !one_of(value(headers, header::HOST), &own.hosts) {
        return fail(
            StatusCode::FORBIDDEN,
            "The Host is not this engine's address.".to_string(),
        );
    }
    if headers.contains_key(header::ORIGIN) && !one_of(value(headers, header::ORIGIN), &own.origins) {
        return fail(
            StatusCode::FORBIDDEN,
            "The request comes from another site.".to_string(),
        );
    }
    // Only the interface: a link on another site may open the page, which does nothing by being read.
    let interface = request.uri().path().starts_with("/api/");
    let made_by = value(headers, "sec-fetch-site");
    if interface && made_by.is_some_and(|site| !matches!(site, "same-origin" | "none")) {
        return fail(
            StatusCode::FORBIDDEN,
            "The request comes from another site.".to_string(),
        );
    }
    // Every POST, even one with no body: a form of another site posts with no leave asked.
    let json = value(headers, header::CONTENT_TYPE)
        .and_then(|kind| kind.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"));
    if request.method() == Method::POST && !json {
        return fail(StatusCode::UNSUPPORTED_MEDIA_TYPE, "A POST carries JSON.".to_string());
    }
    next.run(request).await
}

fn value(headers: &HeaderMap, name: impl header::AsHeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
