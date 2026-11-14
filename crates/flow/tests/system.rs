//! The system prompt of each line of the engineer's flow as the repository defines it (`flow/engineer`), made from
//! the files beside it (`flow/agents`, `flow/project.toml`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use flow::agents::{self, Agents};
use flow::system::{self, Of, Part, Piece, Settings};
use flow::{CellSpec, Compiled, Has, Kind};

/// The directory that holds the repository's flows.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The engineer's flow, compiled against what the engine has for it.
fn engineer() -> Compiled {
    let source = flow::read(&root().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&["answer", "asked", "dependencies", "main", "session", "start", "tree"]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        fields: names(&[
            "dealt_with",
            "meaning_unchanged",
            "observations",
            "questions",
            "reply",
            "summary",
        ]),
        details: names(&["files"]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

fn beside() -> Agents {
    agents::read(&root()).expect("the files beside the flows are read")
}

/// Each file of the project, by its path, as a text that says which it is.
fn project(path: &str) -> Result<Option<String>, String> {
    Ok(Some(format!("The text of {path}.\n")))
}

/// The agent cells of the line.
fn cells<'a>(flow: &'a Compiled, line: &str) -> Vec<&'a CellSpec> {
    (flow.cells.iter())
        .filter(|c| c.kind == Kind::Agent && c.line.as_deref() == Some(line))
        .collect()
}

/// The text of the pieces, each place by its name, as the files write it.
fn as_written(pieces: &[Piece]) -> String {
    (pieces.iter())
        .map(|piece| match piece {
            Piece::Text { text } => text.clone(),
            Piece::Slot { name } => format!("{{{name}}}"),
        })
        .collect()
}

/// The settings the texts name, each with a value.
fn settings(agents: &Agents) -> Settings {
    Settings::of(&agents.project, &agents.installation, |name| {
        Some(format!("the value of {name}"))
    })
}

/// The system prompt as it is sent, its places filled as the engine fills them.
fn sent(parts: &[Part], agents: &Agents) -> String {
    let fill = |name: &str| match name {
        "your_line" => Some("YOUR LINE".to_string()),
        "store" => Some(String::new()),
        "named_checks" => Some("THE CHECKS".to_string()),
        _ => None,
    };
    system::prompt(parts, &settings(agents), fill).expect("the prompt is made")
}

// The system prompt is the line's: every cell of a line is given the same parts, and the same bytes, made again as
// another session of the line begins.
#[test]
fn the_system_prompt_is_the_same_bytes_for_every_cell_of_a_line_and_every_session_of_it() {
    let (flow, agents) = (engineer(), beside());
    let lines: BTreeSet<&str> = flow.cells.iter().filter_map(|c| c.line.as_deref()).collect();
    assert!(lines.contains("engineer") && lines.len() > 1, "{lines:?}");
    for line in lines {
        let of_cell = |cell: &CellSpec| system::parts(&agents, &flow, cell, project).expect("the parts are made");
        let cells = cells(&flow, line);
        let first = sent(&of_cell(cells[0]), &agents);
        for cell in &cells {
            assert_eq!(of_cell(cell), of_cell(cells[0]), "{line}: {}", cell.id.as_str());
            assert_eq!(sent(&of_cell(cell), &agents), first, "{line}: {}", cell.id.as_str());
        }
        // Another session of the line, from the same files read again, begins with the same bytes.
        let again = beside();
        let parts = system::parts(&again, &flow, cells[0], project).expect("the parts are made");
        assert_eq!(sent(&parts, &again), first, "{line}");
    }
    assert!(cells(&flow, "engineer").len() > 1);
}

// The parts stand in the order `session.toml` names them: the project's files, each headed by its path; the sets of
// rules the line takes, in its order; the engine's words; how a cell returns.
#[test]
fn a_lines_system_prompt_holds_the_parts_session_toml_names_in_its_order() {
    let (flow, agents) = (engineer(), beside());
    let parts = system::parts(&agents, &flow, cells(&flow, "engineer")[0], project).unwrap();
    let named: Vec<(Of, &str, &str)> = (parts.iter())
        .map(|p| (p.of, p.name.as_str(), p.source.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            (Of::Project, "guide", "AGENTS.md"),
            (Of::Project, "constitution", "design/constitution.md"),
            (Of::Project, "tree_rules", "design/README.md"),
            (Of::Rules, "general", "flow/agents/rules/general.md"),
            (Of::Rules, "engineering", "flow/agents/rules/engineering.md"),
            (Of::Words, "how_you_work_here.md", "flow/agents/how_you_work_here.md"),
            (Of::Returns, "how_you_return", "flow/agents/return/return.toml"),
        ]
    );
    assert_eq!(as_written(&parts[0].pieces), "# `AGENTS.md`\n\nThe text of AGENTS.md.");
    assert_eq!(as_written(&parts[3].pieces), agents.rules["general"].text.trim_end());
    assert_eq!(
        as_written(&parts[5].pieces),
        agents.words["how_you_work_here.md"].trim_end()
    );
    // Each line is given the sets of rules its file names, in their order, and no other.
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        let parts = system::parts(&agents, &flow, cell, project).unwrap();
        let rules: Vec<&String> = (parts.iter()).filter(|p| p.of == Of::Rules).map(|p| &p.name).collect();
        assert_eq!(rules, cell.system.iter().collect::<Vec<_>>(), "{}", cell.id.as_str());
    }
    // As sent, the engine's words have their place filled, and the settings the texts name, but for the project's
    // files, their values beside them.
    let sent = sent(&parts, &agents);
    assert!(sent.starts_with("# `AGENTS.md`\n\nThe text of AGENTS.md.\n\n# `design/constitution.md`"));
    assert!(
        sent.contains("worktree of the work's own branch. YOUR LINE Never switch"),
        "{sent}"
    );
    assert!(
        sent.contains("`agents.subagents.most` (the value of agents.subagents.most)"),
        "{sent}"
    );
    assert!(
        sent.contains("`owner.language` (the value of owner.language)"),
        "{sent}"
    );
    assert!(!sent.contains("{your_line}") && !sent.contains("{store}"), "{sent}");
}

// How a cell returns holds the words of every field of the engine's and of every tool the flow names, each as its file
// writes it.
#[test]
fn how_you_return_holds_each_fields_and_each_flow_tools_words() {
    let (flow, agents) = (engineer(), beside());
    let told = as_written(&system::how_you_return(&agents, &flow).unwrap());
    assert!(told.starts_with(&agents.returns.how_you_return.heading), "{told}");
    assert!(!agents.fields.is_empty());
    for (name, field) in &agents.fields {
        assert!(told.contains(field.words.trim_end()), "{name}:\n{told}");
    }
    let tools = flow::form::tools(&flow);
    assert_eq!(tools, ["query", "run_tests"]);
    for name in tools {
        assert!(told.contains(agents.tools[name].words.trim_end()), "{name}:\n{told}");
    }
}

// Every setting the texts of every line name has a value where the installation gives each it names one: the prompt is
// made; with one given none, it is not, and the setting is said with the file that holds the words that name it, a
// tool's own file for the words of a tool.
#[test]
fn a_setting_the_texts_name_with_no_value_makes_no_prompt() {
    let (flow, mut agents) = (engineer(), beside());
    let parts = system::parts(&agents, &flow, cells(&flow, "engineer")[0], project).unwrap();
    let fills = |name: &str| ["your_line", "store", "named_checks"].contains(&name);
    assert_eq!(system::check(&parts, &settings(&agents), fills), Vec::<String>::new());
    // The way the project names its tests, which the words of `run_tests` name, given as the installation's here, with
    // no value; and the subagents an agent may start.
    agents.project.project.remove("test_names");
    agents.installation.names.push("project.test_names".to_string());
    let lacking = Settings::of(&agents.project, &agents.installation, |name| {
        (!["agents.subagents.most", "project.test_names"].contains(&name)).then(|| "a value".to_string())
    });
    let fill = |name: &str| Some(name.to_string());
    let wrong = system::prompt(&parts, &lacking, fill).unwrap_err();
    assert_eq!(
        wrong,
        [
            "flow/agents/rules/general.md: names the setting `agents.subagents.most`, which has no value",
            "flow/agents/return/return.toml: names the setting `project.test_names`, which has no value"
        ]
    );
    assert_eq!(
        system::check(&parts, &lacking, fills),
        [
            "flow/agents/rules/general.md: names the setting `agents.subagents.most`, which has no value",
            "flow/agents/tools/run_tests.toml: names the setting `project.test_names`, which has no value"
        ]
    );
}

// What fills the place of a tool's words is the tool's own words for each way the installation runs it, which the
// start checks with the tool's file: the words of `run_tests` say which checks an agent may name by a place, not by a
// setting.
#[test]
fn a_tools_place_is_filled_from_its_own_file() {
    let (flow, agents) = (engineer(), beside());
    let run_tests = &agents.tools["run_tests"];
    assert!(
        run_tests.words.contains("checks among {named_checks};"),
        "{}",
        run_tests.words
    );
    let ways: Vec<&str> = run_tests.places["named_checks"].keys().map(String::as_str).collect();
    assert_eq!(ways, ["named", "none_named", "whole"]);
    assert!(!agents.installation.names.iter().any(|n| n.starts_with("github.")));
    let parts = system::parts(&agents, &flow, cells(&flow, "engineer")[0], project).unwrap();
    let returns = parts.iter().find(|p| p.of == Of::Returns).expect("how a cell returns");
    for words in run_tests.places["named_checks"].values() {
        assert!(
            returns
                .texts
                .iter()
                .any(|t| t.source == "flow/agents/tools/run_tests.toml" && &t.text == words),
            "{words}"
        );
    }
}
