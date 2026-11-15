//! Every kind the routes give of what is wrong with a flow (`What`) and of why a return was sent back (`Unmet`) has the
//! page's words: a kind the page does not know is shown blank, as two of them were (the trigger that can never run its
//! cell, and the run of a line of tests that changed a file of no test).

use std::path::Path;

/// The kinds of the enum of this name in the file, as the routes give them: each variant's name, its first letter
/// small, as `rename_all = "camelCase"` writes the tag.
fn kinds(file: &str, name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).expect("the file reads");
    let open = format!("pub enum {name} {{\n");
    let from = text.find(&open).expect("the enum") + open.len();
    let body = &text[from..from + text[from..].find("\n}\n").expect("its end")];
    let variants: Vec<String> = (body.lines())
        .filter_map(|line| line.strip_prefix("    "))
        .filter(|line| line.starts_with(|c: char| c.is_ascii_uppercase()))
        .map(|line| {
            line.split(|c: char| !c.is_ascii_alphanumeric())
                .next()
                .unwrap_or_default()
        })
        .map(|variant| {
            let mut chars = variant.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_lowercase().to_string() + chars.as_str()
            })
        })
        .collect();
    assert!(variants.len() > 1, "{name} has its variants read: {variants:?}");
    variants
}

/// The kinds of these that the page's file of words has no case of.
fn unworded(kinds: &[String], page: &str) -> Vec<String> {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(page)).expect("the page's words");
    (kinds.iter())
        .filter(|kind| !text.contains(&format!("case '{kind}':")))
        .cloned()
        .collect()
}

// Well under a millisecond alone, two files of the server read beside two of the page; no other test holds the page's
// words to every kind the routes give.
#[test]
fn every_kind_of_fault_and_of_send_back_the_routes_give_has_the_pages_words() {
    let what = kinds("src/view.rs", "What");
    assert_eq!(unworded(&what, "../../client/src/words.ts"), Vec::<String>::new());
    let unmet = kinds("src/works.rs", "Unmet");
    assert_eq!(
        unworded(&unmet, "../../client/src/works/words.ts"),
        Vec::<String>::new()
    );
}
