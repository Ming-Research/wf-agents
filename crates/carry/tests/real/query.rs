//! The tool `query` as the real carriers are made with it: the settings open it, and it is closed unless they do.

use super::*;

#[test]
fn the_store_is_closed_to_agents_unless_the_settings_open_it_and_with_the_real_schema_they_may() {
    let real = Real::trial(Fake::default());
    assert!(!real.settings.query.open, "the store is closed to agents by default");
    let closed = carry::store(&real.settings, &beside()).expect("the tool is closed by default");
    assert!(closed.starts_with("The store is not open to agents yet"), "{closed}");
    // With the real schema, which marks what agents may not read, the engine starts with the store open.
    let mut open = real.settings.clone();
    open.query.open = true;
    let described = carry::store(&open, &beside()).expect("the schema hides what agents may not read");
    assert!(described.contains("### `items`"), "{described}");
    assert!(carry::carriers(open, &real.data, beside()).is_ok(), "the engine starts");
}
