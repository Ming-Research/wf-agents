//! What could come from a page of another site, refused wherever the interface and the page are served: a Host that is
//! not the engine's own address with its port (a name of another site resolved to the loopback address), an Origin
//! that is not the engine's own, a request to the interface that the browser says another site made, and a POST that
//! is not JSON, which a page of another site cannot send without the browser first asking leave. Each request here is
//! sent as bytes to a server listening on the loopback address, so that its headers are exactly those written.

use std::net::Ipv4Addr;

use axum::Router;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::{Shared, get_json, running, serve, tiny};

/// Serves this router on a port of its own on the loopback address, as the engine serves it, and gives the port.
async fn served(router: Router) -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(web::serve(listener, router));
    port
}

/// The flow's routes and the page, served.
async fn flow_page() -> u16 {
    served(serve(&Shared::new(Ok(tiny())))).await
}

/// The engine's own address with this port, as a Host.
fn here(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// The status of the answer to a request with this method, path and these headers, and this body.
async fn status(port: u16, method: &str, path: &str, headers: &[(&str, String)], body: &str) -> u16 {
    let mut request = format!("{method} {path} HTTP/1.1\r\n");
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ));
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    // The status line is all that is read: the stream of changes, let through, never ends.
    let mut answer = Vec::new();
    let mut buffer = [0; 1024];
    while !answer.windows(2).any(|w| w == b"\r\n") {
        let read = stream.read(&mut buffer).await.unwrap();
        assert!(
            read > 0,
            "the server closed without answering: {:?}",
            String::from_utf8_lossy(&answer)
        );
        answer.extend_from_slice(&buffer[..read]);
    }
    let line = String::from_utf8_lossy(&answer).lines().next().unwrap().to_string();
    line.split(' ').nth(1).unwrap().parse().unwrap()
}

/// The status of the answer to a GET with only this Host and these headers.
async fn get(port: u16, path: &str, host: &str, more: &[(&str, &str)]) -> u16 {
    let mut headers = vec![("Host", host.to_string())];
    headers.extend(more.iter().map(|(n, v)| (*n, v.to_string())));
    status(port, "GET", path, &headers, "").await
}

#[tokio::test]
async fn the_engines_own_address_is_served_by_either_name_and_from_its_own_page() {
    let port = flow_page().await;
    let own = format!("http://{}", here(port));
    assert_eq!(get(port, "/api/v1/flow", &here(port), &[]).await, 200);
    assert_eq!(get(port, "/api/v1/flow", &format!("localhost:{port}"), &[]).await, 200);
    assert_eq!(get(port, "/api/v1/flow", &here(port), &[("Origin", &own)]).await, 200);
    let localhost = format!("http://localhost:{port}");
    assert_eq!(
        get(port, "/api/v1/flow", &here(port), &[("Origin", &localhost)]).await,
        200
    );
    for site in ["same-origin", "none"] {
        assert_eq!(
            get(port, "/api/v1/flow", &here(port), &[("Sec-Fetch-Site", site)]).await,
            200,
            "{site}"
        );
    }
    // A link on another site opens the page.
    assert_eq!(
        get(port, "/", &here(port), &[("Sec-Fetch-Site", "cross-site")]).await,
        200
    );
}

#[tokio::test]
async fn a_host_that_is_not_the_engines_own_address_is_refused_on_the_interface_and_the_page() {
    let port = flow_page().await;
    let other = port.wrapping_add(1);
    for path in ["/api/v1/flow", "/"] {
        for host in [
            format!("evil.example:{port}"),
            "evil.example".to_string(),
            format!("127.0.0.1:{other}"),
            format!("localhost:{other}"),
            "127.0.0.1".to_string(),
        ] {
            assert_eq!(get(port, path, &host, &[]).await, 403, "{path} {host}");
        }
        assert_eq!(status(port, "GET", path, &[], "").await, 403, "{path} with no Host");
    }
}

#[tokio::test]
async fn an_origin_that_is_not_the_engines_own_is_refused() {
    let port = flow_page().await;
    let other = port.wrapping_add(1);
    for origin in [
        "https://evil.example".to_string(),
        format!("http://evil.example:{port}"),
        // A page on another port of the loopback address, such as a project's own page an agent wrote.
        format!("http://127.0.0.1:{other}"),
        format!("http://localhost:{other}"),
        format!("https://127.0.0.1:{port}"),
        "null".to_string(),
    ] {
        assert_eq!(
            get(port, "/api/v1/flow", &here(port), &[("Origin", &origin)]).await,
            403,
            "{origin}"
        );
    }
}

#[tokio::test]
async fn a_request_to_the_interface_the_browser_says_another_site_made_is_refused() {
    let port = flow_page().await;
    for site in ["cross-site", "same-site"] {
        assert_eq!(
            get(port, "/api/v1/flow", &here(port), &[("Sec-Fetch-Site", site)]).await,
            403,
            "{site}"
        );
        assert_eq!(
            get(port, "/api/v1/nothing", &here(port), &[("Sec-Fetch-Site", site)]).await,
            403,
            "{site}"
        );
    }
}

#[tokio::test]
async fn every_route_of_the_works_refuses_another_site_and_a_post_that_is_not_json() {
    let running = running();
    let port = served(running.router.clone()).await;
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
    .to_string();
    let host = ("Host", here(port));
    let json = ("Content-Type", "application/json".to_string());
    let evil = ("Origin", "https://evil.example".to_string());
    for kind in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data; boundary=x",
        "application/jsonp",
    ] {
        let headers = [host.clone(), ("Content-Type", kind.to_string())];
        assert_eq!(
            status(port, "POST", "/api/v1/items", &headers, &item).await,
            415,
            "{kind}"
        );
    }
    assert_eq!(
        status(port, "POST", "/api/v1/items", std::slice::from_ref(&host), &item).await,
        415
    );
    let headers = [host.clone(), json.clone(), evil.clone()];
    assert_eq!(status(port, "POST", "/api/v1/items", &headers, &item).await, 403);
    let cross = ("Sec-Fetch-Site", "cross-site".to_string());
    let headers = [host.clone(), json.clone(), cross.clone()];
    assert_eq!(status(port, "POST", "/api/v1/items", &headers, &item).await, 403);
    for path in ["/api/v1/items", "/api/v1/levels", "/api/v1/works/1", "/api/v1/changes"] {
        assert_eq!(
            status(port, "GET", path, &[host.clone(), evil.clone()], "").await,
            403,
            "{path}"
        );
        assert_eq!(
            status(port, "GET", path, &[host.clone(), cross.clone()], "").await,
            403,
            "{path}"
        );
        let rebound = ("Host", format!("evil.example:{port}"));
        assert_eq!(status(port, "GET", path, &[rebound], "").await, 403, "{path}");
    }
    let act = json!({ "kind": "resume" }).to_string();
    let headers = [host.clone(), json.clone(), evil.clone()];
    assert_eq!(status(port, "POST", "/api/v1/works/1/acts", &headers, &act).await, 403);
    let headers = [host.clone(), ("Content-Type", "text/plain".to_string())];
    assert_eq!(status(port, "POST", "/api/v1/works/1/acts", &headers, &act).await, 415);
    let level = json!({ "level": 1 }).to_string();
    let headers = [host.clone(), json.clone(), evil.clone()];
    assert_eq!(
        status(port, "POST", "/api/v1/items/1/level", &headers, &level).await,
        403
    );
    // None of those was taken: the item the owner's own page adds is the first.
    let own = ("Origin", format!("http://{}", here(port)));
    let headers = [
        host,
        ("Content-Type", "application/json; charset=utf-8".to_string()),
        own,
    ];
    assert_eq!(status(port, "POST", "/api/v1/items", &headers, &item).await, 200);
    assert_eq!(
        get_json(&running.router, "/api/v1/items").await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
