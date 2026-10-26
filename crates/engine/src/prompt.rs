//! The text an agent's round is given: the interjections waiting for it, the rules and inputs its session does not
//! hold yet, the cell's question, and what the return must hold, as the JSON Schema the engine checks it against.

use flow::{CompiledCell, CompiledGraph, Held};
use serde_json::{Map, Value};

use crate::Engine;
use crate::inputs::Gathered;
use crate::work::{Item, Work};

/// The columns the table review answers, each with its number, for the review of this round.
pub fn columns(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell, item: &Item) -> Vec<(String, String)> {
    let rule = |name: &str| {
        g.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let mut cols: Vec<String> = Vec::new();
    match cell.id.as_str() {
        "5a" => {
            let table = rule("review_table");
            let project = e.config.config.projects.get(&item.project);
            if item.deliver == "report" {
                cols.extend(table.get(16).cloned());
            } else {
                cols.extend(table.iter().skip(2).take(7).cloned());
                for entry in project.map(|p| p.review_list.clone()).unwrap_or_default() {
                    cols.push(format!("From the review list in the project's configuration: {entry}"));
                }
            }
            let acceptance = item.acceptance.clone().unwrap_or(Value::Null);
            for a in acceptance.as_array().cloned().unwrap_or_default() {
                cols.push(format!(
                    "Acceptance condition {}: {} Can it be checked; is the evidence there, does it come from the head to be merged, and does it hold?",
                    a["id"].as_str().unwrap_or("?"),
                    a["condition"].as_str().unwrap_or_default()
                ));
            }
            if item.deliver != "report" {
                cols.push("The research's judgment that the work stands: does its reason hold?".into());
                cols.push("The judgment that there is no better design: does its reason hold?".into());
                for (c, what) in [
                    ("3a", "a gap in what is under test"),
                    ("3b", "unsure facts"),
                    ("3c", "architectural burden"),
                    ("3d", "cross-project dependency"),
                    ("3e", "splitting"),
                ] {
                    if g.cell(c).is_some() {
                        cols.push(format!(
                            "The check of {what} answered \"no\" or not applicable: does its reason hold?"
                        ));
                    }
                }
                let changes = crate::inputs::last_return(e, w, g, "4", "deviations").ok().flatten();
                for d in changes
                    .and_then(|r| r["deviations"].as_array().cloned())
                    .unwrap_or_default()
                {
                    if d["kind"] != "surprise" {
                        cols.push(format!(
                            "The {} \"{}\": does its reason hold, and did the design change with it?",
                            d["kind"].as_str().unwrap_or_default().replace('_', " "),
                            d["what"].as_str().unwrap_or_default()
                        ));
                    }
                }
                let ends = crate::inputs::last_return(e, w, g, "4", "end").ok().flatten();
                for t in ends
                    .and_then(|r| r["end"]["test_changes"].as_array().cloned())
                    .unwrap_or_default()
                {
                    let name = t["test"].as_str().unwrap_or_default();
                    match t["change"].as_str() {
                        Some("added") => cols.push(format!(
                            "The new test {name}: take the other side on its running time. Do other tests already catch what it catches, could it be smaller or faster, could it be folded into another test?"
                        )),
                        Some(c) => cols.push(format!(
                            "The test {name}, {c}: compared with before, is there a technical reason?"
                        )),
                        None => {}
                    }
                }
            }
        }
        "5b" => {
            let review = rule("design_tree_review");
            for i in [1, 2, 3, 4, 10, 11] {
                cols.extend(review.get(i).cloned());
            }
        }
        _ => {}
    }
    cols.into_iter()
        .enumerate()
        .map(|(i, c)| ((i + 1).to_string(), c))
        .collect()
}

/// Everything the agent's round is given, as one text.
#[allow(clippy::too_many_arguments)]
pub fn agent(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    gathered: &[Gathered],
    interjections: &[Value],
    rejected: Option<&str>,
    given: &Map<String, Value>,
    give_all: bool,
    columns: &[(String, String)],
) -> String {
    let mut out = String::new();
    let mut push = |s: &str| {
        out.push_str(s);
        out.push('\n');
    };
    if !interjections.is_empty() {
        push("# Interjections\n");
        push("Said to you while this work runs; answer them in this round as the return says.\n");
        for i in interjections {
            push(&format!(
                "- From the {} (interjection {}): {}",
                i["speaker"].as_str().unwrap_or("owner"),
                i["interjection"],
                i["text"].as_str().unwrap_or_default()
            ));
        }
        push("");
    }
    let mut rules = String::new();
    for r in &cell.rules {
        if r.held == Held::Session && !give_all {
            continue;
        }
        if let Some(rule) = g.rules.iter().find(|x| x.name == r.name) {
            rules.push_str(&format!(
                "## {}\n\n{}\n\n",
                rule.desc,
                rule.text
                    .iter()
                    .map(|t| format!("- {t}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    if cell.project_rules == Some(Held::Given) || (give_all && cell.project_rules.is_some()) {
        let project = e.config.config.projects.get(&item.project);
        for name in project.map(|p| p.rules.clone()).unwrap_or_default() {
            let Some(text) = e.config.config.rules.get(&name) else {
                continue;
            };
            rules.push_str(text.trim());
            rules.push_str("\n\n");
            let named = e.config.config.named_settings(&item.project, text);
            if !named.is_empty() {
                rules.push_str("The settings these rules name, for this project:\n");
                for (k, v) in named {
                    rules.push_str(&format!("- `{k}`: {v}\n"));
                }
                rules.push('\n');
            }
        }
    }
    if !rules.is_empty() {
        push("# Rules\n");
        push(&rules);
    }
    if give_all && cell.tools.iter().any(|t| t == "query") {
        push("# The database\n");
        push("The tables and views the query tool reads, as the database defines them, with what each holds:\n");
        push(&format!(
            "```sql\n{}```\n",
            db::described(e.db.conn()).unwrap_or_default()
        ));
    }
    push("# What you are given\n");
    if let (Some(wt), Some(branch)) = (&w.worktree, &w.branch) {
        push(&format!(
            "Your working directory is this work's worktree, {}, on the branch {branch}. Commit your changes there; the engine pushes them when you ask it to with a tool.\n",
            wt.display()
        ));
    }
    for x in gathered {
        if x.held() && !give_all {
            continue;
        }
        push(&format!("## {}\n", x.input.what));
        if x.itself() {
            push("Read it yourself.\n");
            continue;
        }
        match &x.value {
            None => push("None.\n"),
            Some(Value::String(s)) => push(&format!("{s}\n")),
            Some(v) => push(&format!(
                "```json\n{}\n```\n",
                serde_json::to_string_pretty(v).unwrap_or_default()
            )),
        }
    }
    if !columns.is_empty() {
        push("## The review table: answer one row for each column, by its number\n");
        for (id, c) in columns {
            push(&format!("{id}. {c}"));
        }
        push("");
    }
    push(&format!("# This step: {}\n", cell.title));
    for p in &cell.prompt {
        push(p);
    }
    if let Some(pb) = &cell.prompt_by
        && let Some(v) = given.get(&pb.by).and_then(Value::as_str)
        && let Some(lines) = pb.cases.get(v)
    {
        for l in lines {
            push(l);
        }
    }
    push("\n# Your return\n");
    if let Some(d) = cell.schema.as_ref().and_then(|s| s["description"].as_str()) {
        push(d);
    }
    push("Hand in one JSON object that fits this JSON Schema, and nothing else:\n");
    let schema = cell
        .schema
        .clone()
        .unwrap_or_else(|| serde_json::json!({ "type": "object" }));
    push(&format!("```json\n{schema}\n```"));
    if let Some(r) = rejected {
        push(&format!(
            "\nYour last return was sent back: {r}\nHand it in again, put right."
        ));
    }
    out
}
