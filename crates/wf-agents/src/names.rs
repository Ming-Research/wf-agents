//! The guard of names: no code of the engine's and none of the page's names a particular cell, line, group or block of
//! a flow (principle engine-knows-no-cell). It compiles every flow the repository defines, collects every name those
//! flows give (each cell, line, group and block, a datum each cell or each line has its own of by its name as made),
//! leaves out what the engine itself declares, read from its declarations (the sources, acts, tools and system parts of
//! what it has, its data beside the flows and their values, every place the words beside the flows fill, the kinds its
//! page's interface tags what it sends with, and the directories of the format), and fails where a source that is not a
//! test holds one of the rest: as a string literal whole, inside a longer one, or as a key of an object written in the
//! page's code, in each Rust file of a crate's `src` outside its items marked `#[cfg(test)]` and the files they
//! declare, and in the page's code without its tests, fixtures, mocks and made-up data, but for those a module of it
//! imports.
//!
//! Its limit: what the engine declares as its own, its acts, sources, tools and the places its words fill, is the
//! engine's vocabulary, which its code may name, so a flow's name that is also one of them (the block `check` and the
//! act `check`, the flow `deliver` and the act `deliver`, the blocks `title`, `code`, `tests` and `checked`, each a
//! place, and the group `change` and the block `merged`, kinds of what the interface sends) is left out, as nothing
//! tells the two apart in a text; the name of an event the page listens for is the browser's word, and is no name of a
//! flow; a plain word given as the index of a value, as in `reply["conclusion"]`, is a field of a reply from outside,
//! read by its sender's word, and is no name of a flow either; and a name that is a plain word, such as `plan` or
//! `found`, is found inside a longer text only where it is written as a name, between backticks, and never as a key,
//! since prose and the code's own words use it for its meaning. A name with a digit or an `_` is no word, and is found
//! wherever it stands, but in the path of a drawing, and a regular expression is no text at all.
//!
//! It reads every source of the repository once and compiles the two flows, 0.9 s on CI. It is worth that: a name
//! written in the code holds for one flow alone, and nothing else finds it until a flow renames the thing and the code
//! silently stops finding it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use flow::{BLOCKS_DIR, CELLS_DIR, DATA_DIR, FLOW_FILE, LINES_DIR, OF_A_CELL, OF_A_LINE, RULES_DIR, TESTS_DIR};

use super::has;

/// The repository's root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A string literal of a source: what stands between its quotes, and the line it begins on, counted from one.
#[derive(Debug, PartialEq, Eq)]
struct Literal {
    text: String,
    line: usize,
}

/// What a source holds that the guard reads: its string literals outside its comments and, in Rust, outside each item
/// marked `#[cfg(test)]`; in the page's code, each key of an object written without quotes; and the modules it declares
/// as tests, each by its name, whose files are tests whole.
#[derive(Debug, Default)]
struct Scanned {
    literals: Vec<Literal>,
    keys: Vec<Literal>,
    tests: Vec<String>,
}

/// Reads a source, Rust or the page's TypeScript. In Rust a literal stands between `"` (a raw string between its own
/// marks), and `'` begins a character or a lifetime; in the page's code it stands between `'`, `"` or `` ` ``, and a
/// `'` or `"` not closed on its line begins none, being an apostrophe of a text or a mark of a regular expression.
struct Scanner {
    chars: Vec<char>,
    at: usize,
    rust: bool,
    /// Where each line begins, by the index of its first character.
    lines: Vec<usize>,
    /// The depth of braces now.
    depth: usize,
    /// The brackets open now, innermost last: `(`, `[` or `{`.
    open: Vec<char>,
    /// Of an item marked `#[cfg(test)]` being read: the depth of braces it began at, and whether its body opened.
    test: Option<(usize, bool)>,
    scanned: Scanned,
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Scanner {
    fn scan(text: &str, rust: bool) -> Scanned {
        let chars: Vec<char> = text.chars().collect();
        let lines = std::iter::once(0)
            .chain(
                chars
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c == '\n')
                    .map(|(i, _)| i + 1),
            )
            .collect();
        let mut scanner = Scanner {
            chars,
            at: 0,
            rust,
            lines,
            depth: 0,
            open: Vec::new(),
            test: None,
            scanned: Scanned::default(),
        };
        scanner.code(false);
        scanner.scanned
    }

    fn get(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn starts_at(&self, i: usize, s: &str) -> bool {
        s.chars().enumerate().all(|(k, x)| self.get(i + k) == Some(x))
    }

    fn starts(&self, s: &str) -> bool {
        self.starts_at(self.at, s)
    }

    fn line(&self, i: usize) -> usize {
        self.lines.partition_point(|&start| start <= i)
    }

    fn found(&mut self, text: String, start: usize) {
        if self.test.is_none() {
            let line = self.line(start);
            self.scanned.literals.push(Literal { text, line });
        }
    }

    /// Reads code up to the end, or, inside a template's `${`, up to the brace that closes it.
    fn code(&mut self, in_template: bool) {
        let mut open = 0usize;
        while let Some(c) = self.get(self.at) {
            if self.starts("//") {
                while self.get(self.at).is_some_and(|c| c != '\n') {
                    self.at += 1;
                }
            } else if self.starts("/*") {
                self.at += 2;
                while self.get(self.at).is_some() && !self.starts("*/") {
                    self.at += 1;
                }
                self.at += 2;
            } else if self.rust && self.starts("#[cfg(test)]") {
                self.at += "#[cfg(test)]".len();
                self.test_module();
                if self.test.is_none() {
                    self.test = Some((self.depth, false));
                }
            } else if c == '"' {
                self.string('"');
            } else if c == '\'' && self.rust {
                self.character();
            } else if c == '\'' {
                self.string('\'');
            } else if c == '`' && !self.rust {
                self.template();
            } else if c == '/' && !self.rust && self.regex_begins() {
                self.regex();
            } else if self.rust && c == 'r' && self.raw_begins() {
                self.raw();
            } else if !self.rust && ident(c) && self.key_begins() {
                self.key();
            } else {
                match c {
                    '(' | '[' => self.open.push(c),
                    ')' | ']' => {
                        self.open.pop();
                    }
                    '{' => {
                        self.open.push(c);
                        open += 1;
                        self.depth += 1;
                        if let Some((at, _)) = self.test {
                            self.test = Some((at, true));
                        }
                    }
                    '}' => {
                        if in_template && open == 0 {
                            self.at += 1;
                            return;
                        }
                        self.open.pop();
                        open = open.saturating_sub(1);
                        self.depth = self.depth.saturating_sub(1);
                        if self.test == Some((self.depth, true)) {
                            self.test = None;
                        }
                    }
                    ';' if self.test == Some((self.depth, false)) => self.test = None,
                    _ => {}
                }
                self.at += 1;
            }
        }
    }

    /// Whether a name here begins a key of an object written without quotes, in the page's code: the innermost bracket
    /// open is a brace, the name comes first in it or after a comma, and a `:` follows it, or `?:`.
    fn key_begins(&self) -> bool {
        if self.open.last() != Some(&'{') || (self.at > 0 && self.get(self.at - 1).is_some_and(ident)) {
            return false;
        }
        let mut i = self.at;
        while i > 0 && self.get(i - 1).is_some_and(char::is_whitespace) {
            i -= 1;
        }
        if !matches!(i.checked_sub(1).and_then(|i| self.get(i)), Some('{' | ',')) {
            return false;
        }
        let mut j = self.at;
        while self.get(j).is_some_and(ident) {
            j += 1;
        }
        if self.get(j) == Some('?') {
            j += 1;
        }
        self.get(j) == Some(':') && self.get(j + 1) != Some(':')
    }

    /// A key of an object written without quotes, kept with the line it stands on.
    fn key(&mut self) {
        let start = self.at;
        while self.get(self.at).is_some_and(ident) {
            self.at += 1;
        }
        if self.test.is_none() {
            let text: String = self.chars[start..self.at].iter().collect();
            let line = self.line(start);
            self.scanned.keys.push(Literal { text, line });
        }
    }

    /// After `#[cfg(test)]`: where it marks `mod <name>;`, the module's name, whose file is a test whole.
    fn test_module(&mut self) {
        let mut i = self.at;
        let blank = |i: &mut usize, s: &Scanner| {
            while s.get(*i).is_some_and(char::is_whitespace) {
                *i += 1;
            }
        };
        let word = |i: &mut usize, s: &Scanner, w: &str| {
            let fits = s.starts_at(*i, w);
            if fits {
                *i += w.chars().count();
            }
            fits
        };
        blank(&mut i, self);
        if word(&mut i, self, "pub(crate)") || word(&mut i, self, "pub ") {
            blank(&mut i, self);
        }
        if !word(&mut i, self, "mod ") {
            return;
        }
        blank(&mut i, self);
        let begins = i;
        while self.get(i).is_some_and(ident) {
            i += 1;
        }
        let name: String = self.chars[begins..i].iter().collect();
        blank(&mut i, self);
        if self.get(i) == Some(';') && !name.is_empty() {
            self.scanned.tests.push(name);
        }
    }

    /// A string between two `quote`s, a `\` taking the character after it; in the page's code one not closed on its
    /// line is none, and reading goes on after its quote.
    fn string(&mut self, quote: char) {
        let start = self.at;
        let mut text = String::new();
        let mut i = self.at + 1;
        while let Some(c) = self.get(i) {
            if c == '\\' {
                text.extend(self.get(i + 1));
                i += 2;
            } else if c == quote {
                self.at = i + 1;
                self.found(text, start);
                return;
            } else if c == '\n' && !self.rust {
                self.at = start + 1;
                return;
            } else {
                text.push(c);
                i += 1;
            }
        }
        self.at = self.chars.len();
    }

    /// In Rust, `'`: a character, whose quotes are passed over whole, or a lifetime, whose mark alone is.
    fn character(&mut self) {
        if self.get(self.at + 1) == Some('\\') {
            let mut i = self.at + 3;
            while self.get(i).is_some_and(|c| c != '\'') {
                i += 1;
            }
            self.at = i + 1;
        } else if self.get(self.at + 2) == Some('\'') {
            self.at += 3;
        } else {
            self.at += 1;
        }
    }

    /// Whether an `r` here begins a raw string: it stands alone or after a `b` that does, and a `"` or a `#` follows.
    fn raw_begins(&self) -> bool {
        let alone = |i: usize| i == 0 || !self.get(i - 1).is_some_and(ident);
        let after_b = self.at >= 1 && self.get(self.at - 1) == Some('b') && alone(self.at - 1);
        (alone(self.at) || after_b) && matches!(self.get(self.at + 1), Some('"' | '#'))
    }

    /// A raw string, from its `r` to its closing `"` and as many `#` as opened it; an `r#` with no `"` after its marks
    /// begins a raw name, which is no literal.
    fn raw(&mut self) {
        let start = self.at;
        let mut i = self.at + 1;
        let mut hashes = 0;
        while self.get(i) == Some('#') {
            hashes += 1;
            i += 1;
        }
        if self.get(i) != Some('"') {
            self.at += 1;
            return;
        }
        i += 1;
        let begins = i;
        while i < self.chars.len() {
            if self.get(i) == Some('"') && (1..=hashes).all(|k| self.get(i + k) == Some('#')) {
                let text = self.chars[begins..i].iter().collect();
                self.at = i + 1 + hashes;
                self.found(text, start);
                return;
            }
            i += 1;
        }
        self.at = self.chars.len();
    }

    /// A template of the page's code, each `${` in it read as code up to its closing brace and standing in the text as
    /// `${}`.
    fn template(&mut self) {
        let start = self.at;
        let mut text = String::new();
        self.at += 1;
        while let Some(c) = self.get(self.at) {
            if c == '\\' {
                text.extend(self.get(self.at + 1));
                self.at += 2;
            } else if c == '`' {
                self.at += 1;
                self.found(text, start);
                return;
            } else if self.starts("${") {
                text.push_str("${}");
                self.at += 2;
                self.code(true);
            } else {
                text.push(c);
                self.at += 1;
            }
        }
    }

    /// Whether a `/` here begins a regular expression in the page's code, not a division: what comes before it, past
    /// blanks, is nothing, a mark after which a value begins, an arrow, or a word after which one does.
    fn regex_begins(&self) -> bool {
        let mut i = self.at;
        while i > 0 && self.get(i - 1).is_some_and(char::is_whitespace) {
            i -= 1;
        }
        let Some(before) = i.checked_sub(1).and_then(|i| self.get(i)) else {
            return true;
        };
        if ident(before) {
            let mut begins = i;
            while begins > 0 && self.get(begins - 1).is_some_and(ident) {
                begins -= 1;
            }
            let word: String = self.chars[begins..i].iter().collect();
            return [
                "return", "typeof", "case", "in", "of", "void", "delete", "throw", "yield", "await",
            ]
            .contains(&word.as_str());
        }
        let arrow = before == '>' && i >= 2 && self.get(i - 2) == Some('=');
        arrow || "(,=:[!&|?{;".contains(before)
    }

    /// A regular expression, from its `/` to the `/` that closes it outside a class, and its flags: no text of the
    /// page's, so that its quotes and backticks open none. One not closed on its line is a division after all.
    fn regex(&mut self) {
        let mut i = self.at + 1;
        let mut class = false;
        while let Some(c) = self.get(i) {
            match c {
                '\\' => i += 1,
                '\n' => break,
                '[' => class = true,
                ']' => class = false,
                '/' if !class => {
                    i += 1;
                    while self.get(i).is_some_and(ident) {
                        i += 1;
                    }
                    self.at = i;
                    return;
                }
                _ => {}
            }
            i += 1;
        }
        self.at += 1;
    }
}

/// Every file under `dir` whose extension is one of `extensions`, each directory's in order.
fn files(dir: &Path, extensions: &[&str], into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, extensions, into);
        } else if path.extension().is_some_and(|e| extensions.iter().any(|x| e == *x)) {
            into.push(path);
        }
    }
}

/// The directories of the flows the repository defines: each under `flow/` that holds a flow's file.
fn flows() -> Vec<PathBuf> {
    let mut flows: Vec<PathBuf> = fs::read_dir(root().join("flow"))
        .expect("the flows' directory is read")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(FLOW_FILE).is_file())
        .collect();
    flows.sort();
    flows
}

/// What of a file beside the flows its words are, in which a name in braces is a place the engine fills: of a TOML
/// file, its lines but its comments; of a text of the engine's words, all of it.
fn filled(path: &Path, text: &str) -> String {
    if path.extension().is_some_and(|e| e == "toml") {
        let words = text.lines().filter(|line| !line.trim_start().starts_with('#'));
        words.collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    }
}

/// Every name in braces the files beside the flows write in their words: the places the engine fills.
fn places() -> BTreeSet<String> {
    fn walk(dir: &Path, into: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if path.is_dir() {
                if !path.join(FLOW_FILE).is_file() {
                    walk(&path, into);
                }
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else { continue };
            let text = filled(&path, &text);
            for piece in text.split('{').skip(1) {
                let Some((name, _)) = piece.split_once('}') else {
                    continue;
                };
                if !name.is_empty() && name.chars().all(ident) {
                    into.insert(name.to_string());
                }
            }
        }
    }
    let mut places = BTreeSet::new();
    walk(&root().join("flow"), &mut places);
    places
}

/// The kinds the page's interface tags what it sends with: each variant of an enum of the web crate tagged by `kind`,
/// named as its `rename_all` names it.
fn interface_kinds() -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    let Ok(entries) = fs::read_dir(root().join("crates/web/src")) else {
        return kinds;
    };
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if !(line.starts_with("#[serde(") && line.contains("tag = \"kind\"")) {
                continue;
            }
            let snake = line.contains("rename_all = \"snake_case\"");
            let Some(_) = lines.by_ref().find(|l| l.starts_with("pub enum ")) else {
                break;
            };
            for line in lines.by_ref().take_while(|l| !l.starts_with('}')) {
                let Some(rest) = line.strip_prefix("    ") else {
                    continue;
                };
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    continue;
                }
                kinds.insert(if snake { snake_cased(&name) } else { camel_cased(&name) });
            }
        }
    }
    kinds
}

/// A variant's name as `rename_all = "camelCase"` writes it.
fn camel_cased(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// A variant's name as `rename_all = "snake_case"` writes it.
fn snake_cased(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Whether a literal is the name of an event the page's code listens for, given to `addEventListener` or
/// `removeEventListener` on its line: the browser's word.
fn listened(lines: &[&str], literal: &Literal) -> bool {
    let line = literal.line.checked_sub(1).and_then(|i| lines.get(i));
    line.is_some_and(|l| {
        ['\'', '"']
            .iter()
            .any(|q| l.contains(&format!("EventListener({q}{}{q}", literal.text)))
    })
}

/// Whether a literal is a plain word given as the index of a value on its line, `reply["conclusion"]`: a field of a
/// reply from outside, which the code reads by its sender's word. A name with a digit or an `_` is never one, and a
/// list of words, `["a", "b"]`, or an array after `!` or `=`, is no index.
fn indexed(lines: &[&str], literal: &Literal) -> bool {
    if no_word(&literal.text) {
        return false;
    }
    let line = literal.line.checked_sub(1).and_then(|i| lines.get(i));
    line.is_some_and(|l| {
        ['\'', '"'].iter().any(|q| {
            let index = format!("[{q}{}{q}]", literal.text);
            l.match_indices(&index).any(|(at, _)| {
                l[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| ident(c) || c == ']' || c == ')')
            })
        })
    })
}

/// The names the flows give that are not the engine's own: each cell, line, group and block of every flow the
/// repository defines, without what the engine declares.
fn flow_names() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut engines = places();
    engines.extend(interface_kinds());
    let format = [
        CELLS_DIR, LINES_DIR, BLOCKS_DIR, TESTS_DIR, RULES_DIR, DATA_DIR, OF_A_CELL, OF_A_LINE,
    ];
    let dirs = format.iter().filter_map(|d| Path::new(d).file_name());
    engines.extend(dirs.map(|d| d.to_string_lossy().into_owned()));
    for dir in flows() {
        let source = flow::read(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let has = has(&dir);
        let compiled = flow::compile(&source, &has).unwrap_or_else(|f| panic!("{}: {f:?}", dir.display()));
        for set in [&has.sources, &has.acts, &has.tools, &has.system] {
            engines.extend(set.iter().cloned());
        }
        for (name, block) in &compiled.blocks {
            if Path::new(&block.file).parent() == Some(Path::new(DATA_DIR)) {
                engines.insert(name.to_string());
                engines.extend(block.values.iter().flatten().cloned());
            }
        }
        names.extend(given(&compiled));
    }
    names.retain(|n| !engines.contains(n));
    names
}

/// The names a flow gives: its own, and each of its cells, lines, groups and blocks.
fn given(compiled: &flow::Compiled) -> BTreeSet<String> {
    let mut names = BTreeSet::from([compiled.name.clone()]);
    names.extend(compiled.cells.iter().map(|c| c.id.as_str().to_string()));
    names.extend(compiled.lines.keys().cloned());
    names.extend(compiled.groups.keys().cloned());
    names.extend(compiled.blocks.keys().map(ToString::to_string));
    names
}

/// The Rust files of the crates that are no tests, each read: each under a crate's `src`, but the file of a module
/// marked `#[cfg(test)]` and every file under it.
fn rust_sources() -> Vec<(PathBuf, Scanned)> {
    let mut all = Vec::new();
    let crates = fs::read_dir(root().join("crates")).expect("the crates are read");
    for krate in crates.filter_map(|e| e.ok()) {
        files(&krate.path().join("src"), &["rs"], &mut all);
    }
    let scanned: Vec<(PathBuf, Scanned)> = (all.into_iter())
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let scanned = Scanner::scan(&text, true);
            (p, scanned)
        })
        .collect();
    let mut tests: Vec<PathBuf> = Vec::new();
    for (path, s) in &scanned {
        let root_of_its_own = path
            .file_name()
            .is_some_and(|n| n == "main.rs" || n == "lib.rs" || n == "mod.rs");
        let base = if root_of_its_own {
            path.parent().map(Path::to_path_buf)
        } else {
            path.parent().zip(path.file_stem()).map(|(p, s)| p.join(s))
        };
        let Some(base) = base else { continue };
        for module in &s.tests {
            tests.push(base.join(format!("{module}.rs")));
            tests.push(base.join(module));
        }
    }
    (scanned.into_iter())
        .filter(|(p, _)| !tests.iter().any(|t| p.starts_with(t)))
        .collect()
}

/// The page's code that is no test, each read: each TypeScript file under `client/src` whose name says no test,
/// fixture, mock or made-up data, and each such file but a test that one of them imports, as the page then runs it.
fn page_sources() -> Vec<(PathBuf, Scanned)> {
    let mut all = Vec::new();
    files(&root().join("client").join("src"), &["ts", "tsx"], &mut all);
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let test = |p: &Path| name(p).contains("test");
    let made_up = |p: &Path| ["fixture", "mock", "fake"].iter().any(|w| name(p).contains(w));
    let mut sources: Vec<PathBuf> = all.iter().filter(|p| !test(p) && !made_up(p)).cloned().collect();
    let mut i = 0;
    while i < sources.len() {
        let text = fs::read_to_string(&sources[i]).unwrap_or_default();
        let dir = sources[i].parent().map(Path::to_path_buf).unwrap_or_default();
        for imported in imports(&text) {
            let path = dir.join(&imported);
            let file = ["ts", "tsx"]
                .iter()
                .map(|e| path.with_extension(e))
                .find(|f| all.contains(f));
            if let Some(file) = file.filter(|f| !test(f) && !sources.contains(f)) {
                sources.push(file);
            }
        }
        i += 1;
    }
    (sources.into_iter())
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let scanned = Scanner::scan(&text, false);
            (p, scanned)
        })
        .collect()
}

/// The modules a file of the page's code imports by a path of its own (`from './mock'`), each as written; what it
/// imports from a package, or only when asked (`import('./fixtures')`), is no part of it as it runs.
fn imports(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let begins = ["import ", "export ", "}"].iter().any(|w| line.starts_with(w));
        let Some(at) = line.find(" from '").filter(|_| begins) else {
            continue;
        };
        let rest = &line[at + " from '".len()..];
        if let Some(path) = rest.split('\'').next().filter(|p| p.starts_with('.')) {
            out.push(path.to_string());
        }
    }
    out
}

/// Whether a name is no word of prose or of the code's own: it holds a digit or an `_`.
fn no_word(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_digit() || c == '_')
}

/// The names a literal holds, as the module says: itself whole; or, inside a longer text, each name written
/// between backticks, and each that is no word, holding a digit or an `_`, wherever it stands. The path of a drawing,
/// a move and then only commands, numbers and their marks, is no text, and holds none.
fn named<'a>(text: &str, names: &'a BTreeSet<String>) -> Vec<&'a str> {
    if let Some(name) = names.get(text) {
        return vec![name.as_str()];
    }
    let drawn = text.starts_with(['M', 'm'])
        && (text.chars()).all(|c| c.is_ascii_digit() || c.is_whitespace() || "MmLlHhVvCcSsQqTtAaZz.,-+".contains(c));
    if drawn {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !(ident(chars[i]) || chars[i] == '.') {
            i += 1;
            continue;
        }
        let begins = i;
        while i < chars.len() && (ident(chars[i]) || chars[i] == '.') {
            i += 1;
        }
        let token: String = chars[begins..i].iter().collect();
        let token = token.trim_end_matches('.');
        let Some(name) = names.get(token) else { continue };
        let quoted = begins > 0 && chars[begins - 1] == '`' && chars.get(begins + token.chars().count()) == Some(&'`');
        if quoted || no_word(token) {
            out.push(name.as_str());
        }
    }
    out
}

#[test]
fn no_source_names_a_cell_line_group_or_block_of_a_flow() {
    let names = flow_names();
    assert!(
        names.contains("plan_owner"),
        "the flows' names are collected: {names:?}"
    );
    let root = root();
    let mut found = Vec::new();
    for (path, scanned) in rust_sources().into_iter().chain(page_sources()) {
        let at = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
        let text = fs::read_to_string(&path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        for literal in scanned
            .literals
            .iter()
            .filter(|l| !listened(&lines, l) && !indexed(&lines, l))
        {
            for name in named(&literal.text, &names) {
                found.push(format!("{at}:{}: {name} in \"{}\"", literal.line, literal.text));
            }
        }
        for key in &scanned.keys {
            for name in named(&key.text, &names).into_iter().filter(|n| no_word(n)) {
                found.push(format!("{at}:{}: {name}, a key", key.line));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these sources name a cell, a line, a group or a block of a flow, which the code reads from the flow's \
         definition instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn a_name_is_found_in_rust_code_and_not_in_its_comments_its_tests_or_its_characters() {
    let text = r####"
// "plan_owner" in a comment
/* "plan_owner" in another */
fn a() -> &'static str { let q = '"'; let e = '\''; "plan_owner" }
#[cfg(test)]
mod tests { fn t() { let x = "in_a_test"; } }
#[cfg(test)]
mod more;
#[cfg(test)]
fn helper() { "in_a_helper"; }
fn b() { let raw = r#"raw "quoted""#; let x = br"bytes"; "after" }
"####;
    let scanned = Scanner::scan(text, true);
    let texts: Vec<(&str, usize)> = scanned.literals.iter().map(|l| (l.text.as_str(), l.line)).collect();
    assert_eq!(
        texts,
        [("plan_owner", 4), (r#"raw "quoted""#, 11), ("bytes", 11), ("after", 11)]
    );
    assert_eq!(scanned.tests, ["more"]);
}

#[test]
fn a_name_is_found_in_the_pages_code_and_not_in_an_apostrophe_or_a_comment() {
    let text = "// 'plan_owner' in a comment\n\
                const a = <p>The owner's words</p>;\n\
                const b = current('plan_owner');\n\
                const c = `${x ? 'inside' : `deeper`}after`;\n\
                const d = /[\"']/;\n\
                const e = \"double\";\n";
    let scanned = Scanner::scan(text, false);
    let texts: Vec<(&str, usize)> = scanned.literals.iter().map(|l| (l.text.as_str(), l.line)).collect();
    assert_eq!(
        texts,
        [
            ("plan_owner", 3),
            ("inside", 4),
            ("deeper", 4),
            ("${}after", 4),
            ("double", 6)
        ]
    );
}

#[test]
fn a_regular_expression_in_the_pages_code_opens_no_text_and_a_division_is_none() {
    let text = "for (const m of t.matchAll(/`([^`\\n/]+)`/g)) f(m);\n\
                const a = `x${y}`;\n\
                const b = n / 2 + m / 3;\n\
                const c = \"after\";\n\
                return /\"[']/.test(s) ? 'yes' : 'no';\n";
    let scanned = Scanner::scan(text, false);
    let texts: Vec<(&str, usize)> = scanned.literals.iter().map(|l| (l.text.as_str(), l.line)).collect();
    assert_eq!(texts, [("x${}", 2), ("after", 4), ("yes", 5), ("no", 5)]);
}

#[test]
fn a_flow_gives_its_own_name_beside_those_of_its_cells_lines_groups_and_blocks() {
    let source = flow::Source {
        flow: "name = \"greeting\"\norder = [\"say\"]\n\n[limits]\nruns = 1\nreturns = 1\ncalls = 1\n\n[blocks.request]\npeople = true\n\n[blocks.said]\npeople = true\n".to_string(),
        cells: [(
            "say".to_string(),
            "title = \"Say\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"request\", \"said\"]\n".to_string(),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let has = flow::Has {
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has).expect("the flow compiles");
    assert_eq!(
        given(&compiled),
        ["greeting", "request", "said", "say"]
            .map(String::from)
            .into_iter()
            .collect()
    );
}

#[test]
fn a_plain_word_given_as_the_index_of_a_value_is_a_field_of_a_reply_and_nothing_else_is() {
    let lines = [
        r#"let a = run["conclusion"].as_str();"#,
        r#"let b = rows[0]["conclusion"];"#,
        r#"let c = ["conclusion", "other"];"#,
        r#"let d = vec!["conclusion"];"#,
        r#"let e = names(["conclusion"]);"#,
        r#"let f = run["plan_owner"];"#,
        r#"let g = run["conclusion_of"]["plan_owner"];"#,
        "const h = run['conclusion'];",
    ];
    let at = |line: usize, text: &str| {
        indexed(
            &lines,
            &Literal {
                text: text.to_string(),
                line,
            },
        )
    };
    assert!(at(1, "conclusion"));
    assert!(at(2, "conclusion"));
    assert!(!at(3, "conclusion"));
    assert!(!at(4, "conclusion"));
    assert!(!at(5, "conclusion"));
    assert!(!at(6, "plan_owner"));
    assert!(!at(7, "plan_owner"));
    assert!(at(8, "conclusion"));
}

#[test]
fn a_key_of_an_object_written_without_quotes_is_found_in_the_pages_code() {
    let text =
        "const w = { R1: 'Research', plan?: 1 };\nfunction f(x: number, y: { z: number }) { return x ? a : b; }\n";
    let scanned = Scanner::scan(text, false);
    let keys: Vec<(&str, usize)> = scanned.keys.iter().map(|k| (k.text.as_str(), k.line)).collect();
    assert_eq!(keys, [("R1", 1), ("plan", 1), ("z", 2)]);
}

#[test]
fn a_name_inside_a_longer_text_is_found_where_it_is_written_as_a_name_or_is_no_word() {
    let names: BTreeSet<String> = ["M1", "plan", "plan_owner"].map(String::from).into_iter().collect();
    assert_eq!(named("waits at M1 now", &names), ["M1"]);
    assert_eq!(named("the plan holds", &names), Vec::<&str>::new());
    assert_eq!(named("`plan` is new", &names), ["plan"]);
    assert_eq!(named("M1.75 2.5h10", &names), Vec::<&str>::new());
    assert_eq!(
        named("M1 6c1.4-2.3 3-3.5 5-3.5S2.4 8.3 1 6z", &names),
        Vec::<&str>::new()
    );
    assert_eq!(named("see plan_owner.", &names), ["plan_owner"]);
    assert_eq!(named("plan", &names), ["plan"]);
}

#[test]
fn a_place_is_a_name_in_braces_the_words_write_and_none_a_comment_writes() {
    let toml = filled(
        Path::new("words.toml"),
        "# {gone} is said in a comment\nkey = \"{kept}\"\n",
    );
    assert!(toml.contains("{kept}") && !toml.contains("{gone}"), "{toml}");
    let words = filled(Path::new("how.md"), "Fill {this} in.");
    assert!(words.contains("{this}"));
}

#[test]
fn a_file_of_made_up_data_a_module_of_the_page_imports_is_read_with_it() {
    let text = "import { a } from './mock';\nimport {\n  b,\n} from '../works/fake';\nimport { c } from 'react';\nconst d = await import('./fixtures');\n";
    assert_eq!(imports(text), ["./mock", "../works/fake"]);
    let read: Vec<PathBuf> = page_sources().into_iter().map(|(p, _)| p).collect();
    assert!(read.iter().any(|p| p.ends_with("console/mock.ts")), "{read:?}");
    assert!(read.iter().any(|p| p.ends_with("console/fake.ts")), "{read:?}");
}
