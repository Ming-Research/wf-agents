//! Everything in the repository is in English: no tracked text file holds a letter that is not ASCII, a character of
//! Unicode's general category L, written as itself or as an escape that stands for one (`\uXXXX`, a pair of them for a
//! letter beyond 16 bits, `\u{...}`, `\UXXXXXXXX`, `\x{...}`, `&#x...;` or `&#...;`), nor a path that does. Data in the
//! owner's language lives in the data directory and the database, never here. Punctuation, symbols and emoji belong to
//! no language and may stay. A file whose language is its point, as a document made as a translation, is exempt when
//! `.config/english-exempt.txt` names it. A file with a NUL byte in its first 8000 bytes is binary, as git takes it,
//! and is not read; any other file is text and must be UTF-8.

use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use regex::{Captures, Regex};

/// The repository's root.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The list of the paths exempt, from the repository's root.
const EXEMPT: &str = ".config/english-exempt.txt";

/// A letter that is not ASCII.
static LETTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}&&[^\x00-\x7F]]").expect("a valid pattern"));

/// An escape that stands for a character, each form in a group of its own: `\UXXXXXXXX`; `\u{...}`; a pair of
/// surrogates, `\uD8XX\uDCXX`; `\uXXXX`; `\x{...}`; `&#x...;`; `&#...;`.
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\\U([0-9A-Fa-f]{8})",
        r"|\\u\{([0-9A-Fa-f]{1,6})\}",
        r"|\\u([dD][89abAB][0-9A-Fa-f]{2})\\u([dD][c-fC-F][0-9A-Fa-f]{2})",
        r"|\\u([0-9A-Fa-f]{4})",
        r"|\\x\{([0-9A-Fa-f]{1,6})\}",
        r"|&#[xX]([0-9A-Fa-f]{1,6});",
        r"|&#([0-9]{1,7});",
    ))
    .expect("a valid pattern")
});

/// The character an escape stands for, when it stands for one.
fn escaped(c: &Captures<'_>) -> Option<char> {
    let hex = |i: usize| c.get(i).and_then(|m| u32::from_str_radix(m.as_str(), 16).ok());
    let point = if let (Some(high), Some(low)) = (hex(3), hex(4)) {
        Some(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00))
    } else if let Some(d) = c.get(8) {
        d.as_str().parse().ok()
    } else {
        [1, 2, 5, 6, 7].into_iter().find_map(hex)
    };
    point.and_then(char::from_u32)
}

/// Each letter that is not ASCII a text holds, as itself or as an escape, with the number of its line.
fn letters(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for m in LETTER.find_iter(line) {
            let c = m.as_str().chars().next().unwrap_or_default();
            found.push((i + 1, format!("U+{:04X}", u32::from(c))));
        }
        for c in ESCAPE.captures_iter(line) {
            if let Some(ch) = escaped(&c).filter(|ch| LETTER.is_match(ch.encode_utf8(&mut [0; 4]))) {
                found.push((i + 1, format!("U+{:04X}, written {}", u32::from(ch), &c[0])));
            }
        }
    }
    found
}

/// Whether an entry of the list of exempt paths covers a path: the same file, or a directory holding it.
fn covers(entry: &str, path: &str) -> bool {
    path == entry || (entry.ends_with('/') && path.starts_with(entry))
}

// Every tracked file, and every path, is in English but those the list exempts, and the list names only paths the
// repository holds.
#[test]
fn every_tracked_file_is_in_english() {
    let root = Path::new(ROOT);
    let list = std::fs::read_to_string(root.join(EXEMPT)).expect("the list of the paths exempt");
    let exempt: Vec<&str> = list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git ls-files: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let files: Vec<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect();
    assert!(files.len() > 100, "git lists {} files", files.len());
    for e in &exempt {
        assert!(
            files.iter().any(|f| covers(e, f)),
            "{EXEMPT} names {e}, which the repository does not hold"
        );
    }
    let mut bad = Vec::new();
    for f in files.iter().filter(|f| !exempt.iter().any(|e| covers(e, f))) {
        bad.extend(
            letters(f)
                .into_iter()
                .map(|(_, what)| format!("{f}: its path holds {what}")),
        );
        let bytes = match std::fs::read(root.join(f)) {
            Ok(b) => b,
            // Deleted and not yet committed.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => panic!("{f}: {e}"),
        };
        if bytes.iter().take(8000).any(|b| *b == 0) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            bad.push(format!("{f}: a text file that is not UTF-8"));
            continue;
        };
        bad.extend(letters(text).into_iter().map(|(n, what)| format!("{f}:{n}: {what}")));
    }
    assert!(
        bad.is_empty(),
        "the repository is in English (AGENTS.md), but these hold a letter that is not ASCII; data in another language \
         belongs in the data directory, and a file whose language is its point is named in {EXEMPT}:\n{}",
        bad.join("\n")
    );
}

// A letter that is not ASCII is found written as itself or as any escape, one beyond 16 bits as a pair of surrogates
// too, on its own line; ASCII, punctuation, symbols and emoji are not, whichever way they are written. The letters are
// made here from their numbers, so that this file holds none.
#[test]
fn a_letter_that_is_not_ascii_is_found_written_any_way() {
    let with = |point: u32| char::from_u32(point).expect("a character").to_string();
    // A small e with an acute accent, and a mathematical bold capital A, beyond 16 bits.
    let (e, a) = (0xE9, 0x1D400);
    for (text, point) in [
        (format!("caf{}", with(e)), e),
        (format!(r"caf\u{e:04x}"), e),
        (format!(r"caf\u{{{e:x}}}"), e),
        (format!(r"caf\U{e:08X}"), e),
        (format!(r"caf\x{{{e:x}}}"), e),
        (format!("caf&#x{e:X};"), e),
        (format!("caf&#{e};"), e),
        (with(a), a),
        (format!(r"\u{:04x}\u{:04x}", 0xD835, 0xDC00), a),
    ] {
        let found = letters(&format!("first line\n{text} here"));
        assert_eq!(found.len(), 1, "{text}: {found:?}");
        assert_eq!(found[0].0, 2, "{text}");
        assert!(found[0].1.starts_with(&format!("U+{point:04X}")), "{text}: {found:?}");
    }
    // An arrow, a curly quote and an emoji, as themselves or escaped, an ASCII letter escaped, and an escape a character
    // cannot have.
    let (arrow, quote, rocket) = (0x2192, 0x201C, 0x1F680);
    for text in [
        format!("{}{}{}", with(arrow), with(quote), with(rocket)),
        format!(r"\u{arrow:04x}\u{{{quote:x}}}\U{rocket:08X}&#x{rocket:X};&#{arrow};"),
        format!(r"\u{:04x}\u{:04x}", 0xD83D, 0xDE80),
        r"A &#65; \x{41} &#x110000; \u{110000}".to_string(),
    ] {
        let found = letters(&text);
        assert!(found.is_empty(), "{text}: {found:?}");
    }
    assert!(covers("docs/", "docs/a.md") && covers("a.md", "a.md") && !covers("docs", "docs/a.md"));
}
