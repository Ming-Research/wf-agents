//! What a test said, as a reader wants it: each line without its control characters but tabs, so that neither the
//! escapes that colour it nor what moves a terminal's cursor reaches the reader, in at most a number of bytes, the first
//! lines and the end kept; and in a fence that makes it read as what the test printed and not as words to its reader.

/// A text an agent's test wrote, as a reader wants it: each line without its control characters but tabs, in at most
/// `budget` bytes, its first lines and its end.
pub(crate) fn plain(text: &str, budget: usize) -> String {
    let lines: Vec<String> = text.trim_end().lines().map(without_controls).collect();
    within(lines, budget).join("\n")
}

/// The text in a fenced block, so that it reads as what the CI printed and not as words to its reader: its fences
/// longer than any run of backquotes it holds.
pub(crate) fn fenced(text: &str) -> String {
    let longest = (text.split(|c| c != '`')).map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}\n{text}\n{fence}")
}

/// What stands where lines are left out.
fn left_out(n: usize) -> String {
    format!("[{n} lines left out]")
}

/// The lines in at most `budget` bytes, each line's end counted: as many of the first as fit in two thirds of it and
/// of the last in the rest, with how many were left out between them; a line too long to fit is cut at a character.
fn within(lines: Vec<String>, budget: usize) -> Vec<String> {
    let size = |ls: &[String]| ls.iter().map(|l| l.len() + 1).sum::<usize>();
    if size(&lines) <= budget {
        return lines;
    }
    let marker = left_out(lines.len()).len() + 1;
    let room = budget.saturating_sub(marker);
    let (mut head, mut used) = (0, 0);
    while head < lines.len() && used + lines[head].len() < room * 2 / 3 {
        used += lines[head].len() + 1;
        head += 1;
    }
    let (mut tail, mut end_used) = (0, 0);
    while tail < lines.len() - head && used + end_used + lines[lines.len() - 1 - tail].len() < room {
        end_used += lines[lines.len() - 1 - tail].len() + 1;
        tail += 1;
    }
    let mut out: Vec<String> = lines[..head].to_vec();
    if head == 0 && tail == 0 {
        // One line longer than the budget: its beginning.
        let mut cut = room.min(lines[0].len());
        while !lines[0].is_char_boundary(cut) {
            cut -= 1;
        }
        out.push(lines[0][..cut].to_string());
        head = 1;
    }
    let left = lines.len() - head - tail;
    if left > 0 {
        out.push(left_out(left));
    }
    out.extend_from_slice(&lines[lines.len() - tail..]);
    out
}

/// A text without its control characters, those of C0 and C1 and delete, but for tabs: an escape sequence that
/// begins with ESC [ or with the C1 introducer is taken out whole, up to the letter that ends it.
fn without_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let sequence = match c {
            '\t' => {
                out.push(c);
                continue;
            }
            '\u{1b}' if chars.peek() == Some(&'[') => {
                chars.next();
                true
            }
            '\u{9b}' => true,
            c if c.is_control() => false,
            c => {
                out.push(c);
                continue;
            }
        };
        if sequence {
            for x in chars.by_ref() {
                if ('@'..='~').contains(&x) {
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_loses_its_colours_and_every_other_control_character_but_tabs() {
        let text = "\u{1b}[1m\u{1b}[91merror\u{1b}[0m:\ttest failed\u{7}\n";
        assert_eq!(plain(text, 1 << 20), "error:\ttest failed");
    }

    // What fits in the bytes given is its first lines and its end, the lines left out said as how many.
    #[test]
    fn a_text_is_given_within_the_bytes_given_its_first_lines_and_its_end() {
        let text: String = (0..100).map(|n| format!("line {n:03}\n")).collect();
        let given = plain(&text, 300);
        assert!(given.len() <= 300, "{}", given.len());
        assert!(
            given.starts_with("line 000\n") && given.ends_with("line 099"),
            "{given}"
        );
        assert!(given.contains(" lines left out]\n"), "{given}");
        let long = "é".repeat(100);
        assert_eq!(plain(&long, 51), "é".repeat(16));
    }

    #[test]
    fn a_fence_is_longer_than_any_run_of_backquotes_in_the_text() {
        assert_eq!(fenced("a `b`"), "```\na `b`\n```");
        assert_eq!(fenced("```rust\nx\n```"), "````\n```rust\nx\n```\n````");
    }
}
