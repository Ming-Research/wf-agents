//! How wide a title is when shown, against Unicode's own classes of characters: UAX #11 (East Asian Width) for how many
//! columns a character takes, and the White_Space property for the characters a title does not count. The letters of
//! other languages are made from their code points, so that the repository holds none.

use config::{title_fits, title_width};

fn made(points: &[u32]) -> String {
    points.iter().map(|p| char::from_u32(*p).unwrap()).collect()
}

// Each character is counted as its class says: wide and full width (W, F) take 2, narrow, half width, neutral and
// ambiguous ones (Na, H, N, A) take 1, and a mark or a format character that takes no column takes 0. The half-width
// kana beside the kana, and the letters of other alphabets beside the Latin ones, are the cases a count by "is it
// ASCII" or by "is it above 255" gets wrong.
#[test]
fn each_character_is_counted_as_its_east_asian_width_class_says() {
    let table: [(u32, usize, &str); 26] = [
        (0x0041, 1, "Latin A (Na)"),
        (0x0060, 1, "grave accent, as the backticks of a name in a title (Na)"),
        (0x0032, 1, "digit two (Na)"),
        (0x00e9, 1, "e with acute (A)"),
        (0x03b1, 1, "Greek alpha (A)"),
        (0x0416, 1, "Cyrillic zhe (A)"),
        (0xff71, 1, "half-width katakana a (H)"),
        (0x20a9, 1, "won sign (N)"),
        (0x4e2d, 2, "Chinese character (W)"),
        (0x3400, 2, "CJK extension A (W)"),
        (0x20000, 2, "CJK extension B (W)"),
        (0x2e80, 2, "CJK radical (W)"),
        (0x3042, 2, "hiragana a (W)"),
        (0x30a2, 2, "katakana a (W)"),
        (0xac00, 2, "hangul syllable (W)"),
        (0x1100, 2, "hangul choseong kiyeok (W)"),
        (0x3002, 2, "ideographic full stop (W)"),
        (0xff0c, 2, "full-width comma (F)"),
        (0xff21, 2, "full-width A (F)"),
        (0xffe6, 2, "full-width won sign (F)"),
        (0x1f680, 2, "rocket emoji (W)"),
        (0x0301, 0, "combining acute accent (Mn)"),
        (0x200d, 0, "zero width joiner (Cf)"),
        (0x200b, 0, "zero width space (Cf)"),
        (0xfe0f, 0, "variation selector 16 (Mn)"),
        (0x0007, 0, "bell (Cc)"),
    ];
    for (point, width, what) in table {
        assert_eq!(title_width(&made(&[point])), width, "{what}: U+{point:04X}");
    }
}

// A title does not count a space of any kind, wherever it stands: every character with the White_Space property,
// put between, before and after the letters of a title, leaves its width as it was.
#[test]
fn no_space_of_any_kind_is_counted_wherever_it_stands() {
    let spaces: [u32; 19] = [
        0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x0020, 0x0085, 0x00a0, 0x1680, 0x2000, 0x2003, 0x200a, 0x2028, 0x2029,
        0x202f, 0x205f, 0x3000, 0x2007, 0x2008,
    ];
    let letters = made(&[0x61, 0x4e2d, 0x62, 0xff71, 0x63]);
    let width = title_width(&letters);
    assert_eq!(width, 1 + 2 + 1 + 1 + 1);
    for space in spaces {
        let s = made(&[space]);
        for spaced in [
            format!("{s}{letters}{s}"),
            letters.chars().map(|c| format!("{s}{c}{s}")).collect::<String>(),
            s.repeat(30) + &letters,
        ] {
            assert_eq!(title_width(&spaced), width, "U+{space:04X}");
        }
        assert_eq!(title_width(&s.repeat(100)), 0, "U+{space:04X}");
    }
}

// The width of two titles written one after the other is the sum of their widths, and a title fits a limit exactly
// when its width is not above it: at every limit from 0 to a little over the width, for English, Chinese and a mixture.
#[test]
fn a_title_fits_exactly_the_limits_not_below_its_width() {
    let chinese = made(&[0x4e2d, 0x6587, 0x6807, 0x9898]);
    let english = "the lexer".to_string();
    let mixed = format!("{english} {chinese}");
    for title in [&english, &chinese, &mixed, &String::new()] {
        let width = title_width(title);
        for limit in 0..width + 3 {
            assert_eq!(title_fits(title, limit).is_ok(), width <= limit, "{title:?} at {limit}");
        }
    }
    assert_eq!(title_width(&mixed), title_width(&english) + title_width(&chinese));
    assert_eq!(title_width(&english), 8);
    assert_eq!(title_width(&chinese), 8);
    // At the limit the owner's installation sets (24): 12 Chinese characters and 24 letters of English fit, 13 and 25
    // do not, and the refusal says how wide the title is.
    let twelve = made(&[0x4e2d; 12]);
    assert_eq!(title_fits(&twelve, 24), Ok(()));
    assert_eq!(title_fits(&"a".repeat(24), 24), Ok(()));
    assert!(
        title_fits(&format!("{twelve}{}", made(&[0x4e2d])), 24)
            .unwrap_err()
            .contains("26")
    );
    assert!(title_fits(&"a".repeat(25), 24).unwrap_err().contains("25"));
    // A refusal says the width, so that a title is shortened by as much as it is over.
    let why = title_fits(&mixed, 10).unwrap_err();
    assert!(why.contains("16"), "{why}");
    assert!(why.contains("10"), "{why}");
}
