//! Reading a flow's directory into the text of its files: laid out one thing to a file, with what it may use beside
//! the flows, or as it was written before.

use std::fs;
use std::path::Path;

use flow::{Source, read};

#[test]
fn a_directory_with_cells_is_read_laid_out_one_thing_to_a_file_with_what_it_may_use_beside_the_flows() {
    let base = std::env::temp_dir().join(format!("flow-read-laid-out-{}", std::process::id()));
    let dir = base.join("one");
    let write = |path: &str, text: &str| {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write("flow.toml", "name = \"one\"");
    write("cells/plan.toml", "title = \"Plan\"");
    write("cells/deep/not.toml", "not directly in cells");
    write("cells/notes.md", "not a cell");
    write("stray.toml", "not a cell, laid out so");
    write("rules/general.md", "no set of rules, laid out so");
    write("lines/engineer.toml", "rules = []");
    write("lines/deep/not.toml", "not directly in lines");
    write("blocks/plan.toml", "");
    write("blocks/judging/deep/opinions.toml", "from = \"owner\"");
    write("blocks/notes.md", "not a block");
    write("tests/settled.toml", "says = \"\"");
    write("../data/request.toml", "from = \"owner\"");
    write("../data/more/answers.toml", "shown = true");
    write("../triggers/on_main.toml", "block = \"main\"");
    write("../triggers/of_a_cell/on_spoken.toml", "block = \"{cell}_spoken_to\"");
    write("../triggers/of_a_datum/changed.toml", "name = \"on_{datum}\"");
    write("../agents/rules/rules.toml", "[general]\nfile = \"general.md\"");
    write("../agents/rules/general.md", "not read with the flow");
    write("../agents/session.toml", "not read with the flow");
    write("../other/flow.toml", "another flow, not read with this one");

    let source = read(&dir);
    fs::remove_dir_all(&base).unwrap();

    let source = source.unwrap();
    assert!(source.laid_out);
    assert_eq!(source.flow, "name = \"one\"");
    assert_eq!(
        source.cells.into_iter().collect::<Vec<_>>(),
        [("plan".to_string(), "title = \"Plan\"".to_string())]
    );
    assert!(source.rules.is_empty());
    assert_eq!(
        source.files.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "../agents/rules/rules.toml",
            "../data/more/answers.toml",
            "../data/request.toml",
            "../triggers/of_a_cell/on_spoken.toml",
            "../triggers/of_a_datum/changed.toml",
            "../triggers/on_main.toml",
            "blocks/judging/deep/opinions.toml",
            "blocks/plan.toml",
            "lines/engineer.toml",
            "tests/settled.toml",
        ]
    );
    assert_eq!(source.files["../data/request.toml"], "from = \"owner\"");
}

#[test]
fn files_by_their_paths_make_a_flow_laid_out_one_thing_to_a_file_as_a_directory_of_them_would() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/laid_out/small");
    let read = read(&dir).unwrap();
    let files = (read.cells.iter())
        .map(|(id, text)| (format!("cells/{id}.toml"), text.clone()))
        .chain(read.files.clone())
        .chain([
            ("flow.toml".to_string(), read.flow.clone()),
            ("stray.toml".to_string(), "not a cell, laid out so".to_string()),
            ("cells/deep/not.toml".to_string(), "not directly in cells".to_string()),
            (
                "../agents/session.toml".to_string(),
                "not read with the flow".to_string(),
            ),
        ]);
    assert_eq!(Source::of_files(files), Some(read));
}

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
