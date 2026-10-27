//! `client/src/api.ts` is generated from the Rust types; a file that differs from what they generate fails here, as
//! a database that differs from `db/schema.sql` would.

use std::path::Path;

#[test]
fn the_page_types_are_what_the_rust_types_generate() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/api.ts");
    let generated = api::typescript();
    if std::env::var_os("WF_AGENTS_GENERATE").is_some() {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        kept == generated,
        "{} is not what the Rust types generate: run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript`",
        path.display()
    );
}
