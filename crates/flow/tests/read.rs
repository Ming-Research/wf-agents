//! Reading a flow's directory into the text of its files.

use std::fs;

use flow::{Source, read};

#[test]
fn a_directory_is_read_as_flow_toml_and_one_file_for_each_cell_named_by_the_file() {
    let dir = std::env::temp_dir().join(format!("flow-read-{}", std::process::id()));
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::write(dir.join("flow.toml"), "name = \"small\"").unwrap();
    fs::write(dir.join("plan.toml"), "title = \"Plan\"").unwrap();
    fs::write(dir.join("notes.md"), "not a cell").unwrap();
    fs::write(dir.join("nested").join("deep.toml"), "not a cell either").unwrap();

    let source = read(&dir);
    fs::remove_dir_all(&dir).unwrap();

    let mut expected = Source {
        flow: "name = \"small\"".to_string(),
        ..Source::default()
    };
    expected
        .cells
        .insert("plan".to_string(), "title = \"Plan\"".to_string());
    assert_eq!(source.unwrap(), expected);
}
