//! How long an item's title may be, in the unit the installation's `items.title_max` is in.

/// How many words a title has, the unit `items.title_max` is in. A run of characters between spaces is one word where
/// it holds a letter or a digit; a letter of East Asian wide or full width (a Chinese character, a kana, a hangul
/// syllable) is a word of its own and ends the run before it, so a Chinese title is counted by its characters. A space
/// of any kind, punctuation and a mark standing alone are no words.
pub fn title_words(title: &str) -> usize {
    let mut words = 0;
    let mut run_has_letter = false;
    for c in title.chars() {
        let wide = unicode_width::UnicodeWidthChar::width(c) == Some(2);
        if c.is_whitespace() || wide {
            words += usize::from(run_has_letter);
            run_has_letter = false;
            words += usize::from(wide && c.is_alphanumeric());
        } else if c.is_alphanumeric() {
            run_has_letter = true;
        }
    }
    words + usize::from(run_has_letter)
}

/// Whether a title is within `max` words ([`title_words`]); if not, why not, saying how many it has.
pub fn title_fits(title: &str, max: usize) -> Result<(), String> {
    let words = title_words(title);
    if words > max {
        return Err(format!(
            "the title has {words} words and a title has {max} at most (a Chinese character counts as a word): write \
             it shorter"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{title_fits, title_words};

    // Each kind of character counts as the rule says: a run between spaces is a word where it holds a letter or a
    // digit, whatever joins its letters; a wide letter is a word of its own, also inside a run; wide punctuation, an
    // emoji, narrow punctuation standing alone and a mark add none. The half-width kana beside the kana, and the
    // letters of other alphabets beside the Latin ones, are the cases a count by "is it ASCII" gets wrong.
    #[test]
    fn each_kind_of_character_counts_as_the_rule_says() {
        let table: [(&str, usize, &str); 20] = [
            ("", 0, "nothing"),
            ("the lexer", 2, "two English words"),
            ("fast.yml", 1, "a name with a dot"),
            ("a-b_c", 1, "letters joined by marks of punctuation"),
            ("#4", 1, "a number"),
            ("`run_tests`", 1, "a name in backticks"),
            ("a / b", 2, "a slash standing alone"),
            ("\u{4e2d}\u{6587}\u{6807}\u{9898}", 4, "four Chinese characters"),
            (
                "\u{4e2d}\u{6587}\u{ff0c}\u{6807}\u{9898}",
                4,
                "a full-width comma between them",
            ),
            ("\u{4e2d}\u{3002}", 1, "an ideographic full stop"),
            ("run_tests \u{7ed9}\u{51fa} job", 4, "English names around Chinese"),
            (
                "job\u{7684}\u{8017}\u{65f6}",
                4,
                "a Chinese character ends the run before it",
            ),
            ("\u{3042}\u{3044}", 2, "two hiragana"),
            ("\u{d55c}\u{ad6d}", 2, "two hangul syllables"),
            ("\u{ff71}\u{ff72}\u{ff73}", 1, "half-width katakana, one run"),
            ("\u{3b1}\u{3b2}\u{3b3} \u{3b4}", 2, "Greek words"),
            ("\u{ff21}\u{ff22}", 2, "full-width letters"),
            ("\u{1f680} launch", 1, "an emoji"),
            ("e\u{301}t\u{e9}", 1, "a combining accent inside a word"),
            ("x\u{200d}y \u{301}", 1, "a joiner inside a word, an accent alone"),
        ];
        for (title, words, what) in table {
            assert_eq!(title_words(title), words, "{what}: {title:?}");
        }
    }

    // A space of any kind counts nothing and only separates: every character with the White_Space property, one or
    // many, before, after and between the words of a title, leaves its count as it was, and a title of spaces alone
    // has no word.
    #[test]
    fn no_space_of_any_kind_is_counted_wherever_it_stands() {
        let spaces: [u32; 19] = [
            0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x0020, 0x0085, 0x00a0, 0x1680, 0x2000, 0x2003, 0x200a, 0x2028,
            0x2029, 0x202f, 0x205f, 0x3000, 0x2007, 0x2008,
        ];
        let words = ["run_tests", "\u{4e2d}", "\u{6587}", "job", "\u{ff71}\u{ff72}"];
        for space in spaces {
            let s = char::from_u32(space).unwrap().to_string();
            for between in [s.clone(), s.repeat(3)] {
                let spaced = format!("{between}{}{between}", words.join(&between));
                assert_eq!(title_words(&spaced), words.len(), "U+{space:04X}");
            }
            assert_eq!(title_words(&s.repeat(100)), 0, "U+{space:04X}");
        }
    }

    // The count of two titles written one after the other with a space between is the sum of their counts, and a
    // title fits a limit exactly when its count is not above it: at every limit from 0 to a little over the count, for
    // English, Chinese and a mixture.
    #[test]
    fn a_title_fits_exactly_the_limits_not_below_its_count() {
        let chinese = "\u{4e2d}\u{6587}\u{6807}\u{9898}".to_string();
        let english = "the lexer".to_string();
        let mixed = format!("{english} {chinese}");
        for title in [&english, &chinese, &mixed, &String::new()] {
            let words = title_words(title);
            for limit in 0..words + 3 {
                assert_eq!(title_fits(title, limit).is_ok(), words <= limit, "{title:?} at {limit}");
            }
        }
        assert_eq!(title_words(&mixed), title_words(&english) + title_words(&chinese));
        // At a limit of 20: 20 Chinese characters and 20 English words fit, 21 do not, and the refusal says how many
        // words the title has and the limit, so that a title is shortened by as much as it is over.
        let twenty = "\u{4e2d}".repeat(20);
        assert_eq!(title_fits(&twenty, 20), Ok(()));
        assert_eq!(title_fits("word ".repeat(20).trim(), 20), Ok(()));
        let why = title_fits(&format!("{twenty}\u{4e2d}"), 20).unwrap_err();
        assert!(why.contains("21") && why.contains("20"), "{why}");
        assert!(title_fits("word ".repeat(21).trim(), 20).unwrap_err().contains("21"));
    }
}
