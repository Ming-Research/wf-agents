//! The engineer's flow, as it is defined in the repository, compiles.

use std::collections::BTreeSet;
use std::path::Path;

use decide::TestName;
use flow::{compile, read};

#[test]
fn the_engineers_flow_compiles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = read(&dir).expect("the definition can be read");
    // The engine has no named tests yet: the ones the definition declares are taken as those it has.
    let declared: toml::Table = toml::from_str(&source.flow).expect("flow.toml is TOML");
    let known: BTreeSet<TestName> = (declared.get("tests").and_then(|t| t.as_table()).into_iter())
        .flat_map(|t| t.keys())
        .map(|name| TestName(name.clone()))
        .collect();
    if let Err(faults) = compile(&source, &known) {
        panic!("the engineer's flow has faults: {faults:#?}");
    }
}
