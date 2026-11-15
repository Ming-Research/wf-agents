//! A flow with every name it gives renamed alike: for the properties that hold whatever a flow names, its exploration's
//! and the build of the page's states (crate `wf-agents`), each of which names no cell, block or trigger, so that the
//! flow renamed goes every way the flow does, its names following.

use std::collections::BTreeMap;
use std::path::Path;

use flow::Source;

/// The source with every cell, block, group, line, named test and trigger of the flow given another name, and the names
/// given, each by the one it replaces: the trigger of a block's or a group's new version (`on_` and its name) follows
/// its name, each block a trigger makes for a cell or a line (`{cell}_tested`, `{line}_session`) its cell's or its
/// line's, and a block a trigger makes under a name of its own (`main`) is renamed as a block is.
pub fn renamed(source: &Source) -> (BTreeMap<String, String>, Source) {
    let mut names = BTreeMap::new();
    // One prefix for every name, after `on_` for a trigger's, keeps every set of names in the order it had, so that the
    // exploration, which goes through them in that order, goes the same way.
    let new = |name: &str| match name.strip_prefix("on_") {
        Some(rest) => format!("on_x_{rest}"),
        None => format!("x_{name}"),
    };
    for cell in source.cells.keys() {
        names.insert(cell.clone(), new(cell));
    }
    for path in source.files.keys() {
        let stem = (Path::new(path).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if path.starts_with("blocks/") || path.starts_with("../data/") {
            names.insert(stem.to_string(), new(stem));
            names.insert(format!("on_{stem}"), format!("on_{}", new(stem)));
        } else if (path.starts_with("../triggers/") && !path.ends_with("/changed.toml"))
            || path.starts_with("lines/")
            || path.starts_with("tests/")
        {
            names.insert(stem.to_string(), new(stem));
        }
    }
    // A group's name, and its trigger, `on_` and its name.
    let mut groups = false;
    for line in source.flow.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            groups = line == "[groups]";
        } else if groups && let Some((group, _)) = line.split_once('=') {
            let group = group.trim();
            names.insert(group.to_string(), new(group));
            names.insert(format!("on_{group}"), new(&format!("on_{group}")));
        }
    }
    // The blocks a trigger's file makes, by its `block`, for each cell (`{cell}`, the cell's id in small letters) or
    // line (`{line}`), as the compiler names them: each renamed as its cell or line is.
    let cells: Vec<String> = source.cells.keys().cloned().collect();
    let lines: Vec<String> = (source.files.keys())
        .filter(|path| path.starts_with("lines/"))
        .filter_map(|path| Path::new(path).file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect();
    for (path, text) in &source.files {
        if !path.starts_with("../triggers/") {
            continue;
        }
        for line in text.lines() {
            let template = (line.trim().strip_prefix("block = \"")).and_then(|t| t.strip_suffix('"'));
            let Some(template) = template else {
                continue;
            };
            // A block a trigger makes under a name of its own, as the trigger of main makes main.
            if !template.contains('{') {
                names.insert(template.to_string(), new(template));
            }
            for (place, of) in [("{cell}", &cells), ("{line}", &lines)] {
                if template.contains(place) {
                    for name in of {
                        let was = template.replace(place, &name.to_lowercase());
                        let is = template.replace(place, &new(name).to_lowercase());
                        names.insert(was, is);
                    }
                }
            }
        }
    }
    let file = |path: &String| {
        let stem = (Path::new(path).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        match names.get(stem) {
            Some(new) if path.ends_with(&format!("/{stem}.toml")) => {
                format!("{}/{new}.toml", &path[..path.len() - stem.len() - "/.toml".len()])
            }
            _ => path.clone(),
        }
    };
    let renamed = Source {
        laid_out: source.laid_out,
        flow: rename_text(&source.flow, &names),
        cells: (source.cells.iter())
            .map(|(id, text)| (names[id].clone(), rename_text(text, &names)))
            .collect(),
        rules: source.rules.clone(),
        files: (source.files.iter())
            .map(|(path, text)| (file(path), rename_text(text, &names)))
            .collect(),
    };
    (names, renamed)
}

/// The text with each word the names give renamed, but where the word names no cell, block or trigger: a key of the
/// file's own form (a key at the start of a line, but in a table whose keys are names: `kinds`, `do`, `groups`), a
/// source or an act (`outside`, `from`, `act`), a place a text fills (`{name}`), a value of a choice (a list of
/// `values`, or what a key of a block is set to), and the line of a table's name.
fn rename_text(text: &str, names: &BTreeMap<String, String>) -> String {
    const NAMED: [&str; 3] = ["kinds", "do", "groups"];
    let mut out = String::with_capacity(text.len());
    let mut table = String::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') && !trimmed.starts_with("[\"") {
            table = trimmed.trim_end().trim_matches(|c| c == '[' || c == ']').to_string();
            out.push_str(line);
            continue;
        }
        if trimmed.starts_with("values") {
            out.push_str(line);
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let word_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let mut i = 0;
        while i < chars.len() {
            if !word_char(chars[i]) {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let before: String = chars[..start].iter().collect();
            let after: String = chars[i..].iter().collect();
            let key = after.trim_start().starts_with('=') && !after.trim_start().starts_with("==");
            let form = key && before.trim().is_empty() && !NAMED.contains(&table.as_str());
            let value_of_a_block = (before.strip_suffix("= \"").map(str::trim_end)).is_some_and(|key| {
                let at = key.rfind(|c: char| !word_char(c)).map_or(0, |at| at + 1);
                names.contains_key(&key[at..])
            });
            let kept = form
                || before.ends_with('{')
                || ["outside = \"", "from = \"", "act = \""]
                    .iter()
                    .any(|k| before.ends_with(k))
                || value_of_a_block;
            match names.get(&word) {
                Some(new) if !kept => out.push_str(new),
                _ => out.push_str(&word),
            }
        }
    }
    out
}
