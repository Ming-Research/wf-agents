//! How the command a project's configuration holds is split into words: `config::words` says it splits "as a shell
//! would", and the engine's tools run the words without a shell, so a word that differs from what a shell makes of the
//! same line is a command run with an argument the project did not write.

// In double quotes a backslash escapes only `$`, a backtick, `"`, `\` and a newline; before any other character it is
// itself a character of the word (POSIX Shell Command Language, 2.2.3 Double-Quotes). So `sh -c "printf 'a\n'"` hands
// printf the two characters `\n`, which is what a project that writes it means; dropping the backslash hands it `n`.
#[test]
fn a_backslash_in_double_quotes_stays_unless_it_escapes_what_the_shell_lets_it() {
    // Each line with the words a POSIX shell makes of it.
    let lines: [(&str, &[&str]); 4] = [
        (r#"sh -c "printf 'a\n'""#, &["sh", "-c", r"printf 'a\n'"]),
        (r#"echo "a\tb" "c\\d""#, &["echo", r"a\tb", r"c\d"]),
        (r#"echo "x\$y" "q\"r""#, &["echo", "x$y", r#"q"r"#]),
        (r#"echo 'a\nb'"#, &["echo", r"a\nb"]),
    ];
    for (line, expected) in lines {
        assert_eq!(config::words(line).unwrap(), expected, "{line}");
    }
}

// A backslash that ends the line escapes nothing and is kept as the character it is (POSIX 2.2.1 Escape Character: it
// is only removed before a newline). A line that ends in a backslash is not one the shell reads as a shorter one.
#[test]
fn a_backslash_that_ends_the_line_is_a_character_of_its_word() {
    assert_eq!(config::words(r"echo a\").unwrap(), ["echo", r"a\"]);
    assert_eq!(config::words(r"a\").unwrap(), [r"a\"]);
}
