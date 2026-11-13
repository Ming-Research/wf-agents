//! Reading a flow's directory into the text of its files.

use std::fs;

use flow::{Source, read};

#[test]
fn a_directory_is_read_as_flow_toml_one_file_for_each_cell_and_one_for_each_set_of_rules_named_by_the_file() {
    let dir = std::env::temp_dir().join(format!("flow-read-{}", std::process::id()));
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::create_dir_all(dir.join("rules").join("old")).unwrap();
    fs::write(dir.join("flow.toml"), "name = \"small\"").unwrap();
    fs::write(dir.join("plan.toml"), "title = \"Plan\"").unwrap();
    fs::write(dir.join("notes.md"), "not a cell").unwrap();
    fs::write(dir.join("nested").join("deep.toml"), "not a cell either").unwrap();
    fs::write(dir.join("rules").join("general.md"), "Say one thing per sentence.").unwrap();
    fs::write(dir.join("rules").join("general.toml"), "not a set of rules").unwrap();
    fs::write(
        dir.join("rules").join("old").join("general.md"),
        "not directly in rules",
    )
    .unwrap();

    let source = read(&dir);
    fs::remove_dir_all(&dir).unwrap();

    let mut expected = Source {
        flow: "name = \"small\"".to_string(),
        ..Source::default()
    };
    expected
        .cells
        .insert("plan".to_string(), "title = \"Plan\"".to_string());
    expected
        .rules
        .insert("general".to_string(), "Say one thing per sentence.".to_string());
    assert_eq!(source.unwrap(), expected);
}

#[test]
fn a_directory_without_rules_has_no_sets_of_rules() {
    let dir = std::env::temp_dir().join(format!("flow-read-no-rules-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("flow.toml"), "name = \"small\"").unwrap();

    let source = read(&dir);
    fs::remove_dir_all(&dir).unwrap();

    assert!(source.unwrap().rules.is_empty());
}

#[test]
fn files_by_their_paths_make_the_flow_as_a_directory_of_them_would() {
    let files = [
        ("flow.toml", "name = \"small\""),
        ("plan.toml", "title = \"Plan\""),
        ("notes.md", "not a cell"),
        ("nested/deep.toml", "not a cell either"),
        ("rules/general.md", "Say one thing per sentence."),
        ("rules/general.toml", "not a set of rules"),
        ("rules/old/general.md", "not directly in rules"),
    ];
    let source = Source::of_files(files.map(|(path, text)| (path.to_string(), text.to_string()))).unwrap();
    assert_eq!(source.flow, "name = \"small\"");
    assert_eq!(
        source.cells.into_iter().collect::<Vec<_>>(),
        [("plan".to_string(), "title = \"Plan\"".to_string())]
    );
    assert_eq!(
        source.rules.into_iter().collect::<Vec<_>>(),
        [("general".to_string(), "Say one thing per sentence.".to_string())]
    );
    // Without flow.toml there is no flow.
    assert_eq!(Source::of_files([("plan.toml".to_string(), String::new())]), None);
}
