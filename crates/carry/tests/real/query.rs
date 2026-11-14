//! The tool `query` as the real carriers are made with it: the settings open it, and it is closed unless they do.

use super::*;

#[test]
fn the_store_is_open_to_agents_only_where_the_settings_open_it_and_the_schema_hides_something() {
    let real = Real::trial(Fake::default());
    let closed = carry::store(&real.settings, &beside()).expect("the tool is closed by default");
    assert!(closed.starts_with("The store is not open to agents yet"), "{closed}");
    let mut open = real.settings.clone();
    open.query.open = true;
    // The real schema marks nothing hidden from agents yet: opening the store is a fault at the start, which the
    // carriers are made with the same settings to say.
    let why = carry::store(&open, &beside()).unwrap_err();
    assert!(why.starts_with("query.open: "), "{why}");
    assert!(why.contains("hidden from agents"), "{why}");
    let refused = carry::carriers(open, &real.data, beside())
        .err()
        .expect("the carriers are not made");
    assert_eq!(refused, why);
}
