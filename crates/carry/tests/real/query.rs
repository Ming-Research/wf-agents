//! The tool `query` as the real carriers are made with it: always open, with no setting.

use super::*;

#[test]
fn the_store_is_always_open_to_agents_and_the_engine_starts_with_it_whatever_the_settings() {
    let real = Real::trial(Fake::default());
    // With the real schema, which marks what agents may not read, the tool describes the store.
    let described = carry::store(&beside());
    assert!(described.contains("### `items`"), "{described}");
    let flows = [real.flow.clone()];
    assert!(
        carry::carriers(real.settings.clone(), &real.data, beside(), &flows).is_ok(),
        "the engine starts"
    );
}
