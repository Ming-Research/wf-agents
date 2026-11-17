//! What a run of the made project's tests gives where a case's cell asks for one (design document `testing`, 13): the
//! tests of the commit run, found in its files as `[tests]` of `common.toml` says; each failing where a rule of the
//! case's says it does and passing otherwise; and the JUnit file a run of its workflow uploads, which the engine reads as
//! it reads a real run's. Rules only approximate what a run of the code would find: a test fails by its name, whether
//! main has it, what the commit's files hold and what the test's own text holds, never by evaluating what it asserts.

use crate::written::{Rules, Tests};

/// A test of the commit run: its unit, as the made project's runner names the test binary it is in, its name, and its
/// text, from its line to the next test's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    pub unit: String,
    pub name: String,
    pub text: String,
}

/// The tests of these files, each a path and its content, as the made project's tests are found. A file under
/// `crates/<crate>/tests/` is the unit `<crate>::<file>`, and any other under `crates/<crate>/` the unit `<crate>`, as
/// the runner the made project's workflows use names its test binaries.
pub fn tests_in(files: &[(String, String)], how: &Tests) -> Vec<Test> {
    let mut found = Vec::new();
    for (path, content) in files {
        if !how.under.iter().any(|dir| path.starts_with(dir.as_str())) || !path.ends_with(".rs") {
            continue;
        }
        let unit = unit_of(path);
        let lines: Vec<&str> = content.lines().collect();
        let marks: Vec<usize> = (0..lines.len())
            .filter(|i| lines[*i].contains(how.mark.as_str()))
            .collect();
        for (k, at) in marks.iter().enumerate() {
            let end = marks.get(k + 1).copied().unwrap_or(lines.len());
            let named = (*at..end).find_map(|i| {
                let after = lines[i].split_once(how.name_after.as_str())?.1;
                let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                (!name.is_empty()).then_some(name)
            });
            if let Some(name) = named {
                found.push(Test {
                    unit: unit.clone(),
                    name,
                    text: lines[*at..end].join("\n"),
                });
            }
        }
    }
    found
}

fn unit_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [_, krate, "tests", file] => format!("{krate}::{}", file.trim_end_matches(".rs")),
        [_, krate, ..] => (*krate).to_string(),
        _ => path.to_string(),
    }
}

/// Whether the name fits the pattern, where `*` stands for any text.
pub fn fits(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, rest) = parts.split_first().map_or(("", &[][..]), |(f, r)| (*f, r));
    let Some(mut left) = name.strip_prefix(first) else {
        return false;
    };
    for (i, part) in rest.iter().enumerate() {
        let last = i + 1 == rest.len();
        match (last, left.find(part)) {
            (true, _) => return left.ends_with(part),
            (false, Some(at)) => left = &left[at + part.len()..],
            (false, None) => return false,
        }
    }
    left.is_empty()
}

/// What each test does: none where it passes, what it says where it fails. `main` names the tests main has; `file`
/// gives the content of a file of the commit run.
pub fn outcomes(
    rules: &Rules,
    tests: &[Test],
    main: &[String],
    file: impl Fn(&str) -> Option<String>,
) -> Vec<Option<String>> {
    (tests.iter())
        .map(|test| {
            let fails = rules.fails.iter().find(|rule| {
                let named = rule.names.iter().any(|n| fits(n, &test.name));
                let new = rule.new.is_none_or(|new| new != main.contains(&test.name));
                let holds = rule.holds.iter().all(|h| test.text.contains(h.as_str()));
                let at = rule.at.as_ref().is_none_or(|at| {
                    let content = file(&at.file).unwrap_or_default();
                    let has = at.has.as_ref().is_none_or(|h| content.contains(h.as_str()));
                    let lacks = at.lacks.as_ref().is_none_or(|l| !content.contains(l.as_str()));
                    file(&at.file).is_some() && has && lacks
                });
                named && new && holds && at
            });
            fails.map(|rule| rule.said.clone())
        })
        .collect()
}

/// The JUnit file a run of these tests uploads, each with what it does.
pub fn junit(tests: &[Test], outcomes: &[Option<String>]) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let mut units: Vec<&str> = Vec::new();
    for test in tests {
        if !units.contains(&test.unit.as_str()) {
            units.push(&test.unit);
        }
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n");
    for unit in units {
        xml.push_str(&format!("  <testsuite name=\"{}\">\n", escape(unit)));
        for (test, outcome) in tests.iter().zip(outcomes).filter(|(t, _)| t.unit == unit) {
            let name = escape(&test.name);
            match outcome {
                None => xml.push_str(&format!("    <testcase name=\"{name}\" time=\"0.002\"/>\n")),
                Some(said) => xml.push_str(&format!(
                    "    <testcase name=\"{name}\" time=\"0.003\"><failure>{}</failure></testcase>\n",
                    escape(said)
                )),
            }
        }
        xml.push_str("  </testsuite>\n");
    }
    xml.push_str("</testsuites>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::written::{Fails, Where};

    fn how() -> Tests {
        Tests {
            mark: "#[test]".into(),
            name_after: "fn ".into(),
            under: vec!["crates/".into()],
            full: vec!["test".into()],
            fast: "named".into(),
            seconds: 40,
            url: "https://ci.example.invalid/runs/".into(),
        }
    }

    #[test]
    fn a_pattern_fits_a_name_with_any_text_where_it_has_a_star() {
        assert!(fits("*", "anything"));
        assert!(fits("monthly_*", "monthly_keeps_months_apart"));
        assert!(fits("*_apart", "monthly_keeps_months_apart"));
        assert!(fits("*keeps*", "monthly_keeps_months_apart"));
        assert!(fits("monthly_keeps_months_apart", "monthly_keeps_months_apart"));
        assert!(!fits("monthly", "monthly_keeps_months_apart"));
        assert!(!fits("*_together", "monthly_keeps_months_apart"));
        assert!(!fits("a*b*c", "a_c_b"));
    }

    #[test]
    fn a_test_fails_where_a_rule_holds_of_it_and_of_the_commit_and_passes_otherwise() {
        let files = vec![
            (
                "crates/tally/tests/report.rs".to_string(),
                "use x;\n#[test]\nfn monthly_sums() {\n    monthly(&l);\n}\n\n#[test]\nfn renders() {}\n".to_string(),
            ),
            (
                "crates/tally/src/lib.rs".to_string(),
                "#[test]\nfn inner() {}\n".to_string(),
            ),
            ("examples/x.rs".to_string(), "#[test]\nfn outside() {}\n".to_string()),
        ];
        let tests = tests_in(&files, &how());
        let names: Vec<(&str, &str)> = tests.iter().map(|t| (t.unit.as_str(), t.name.as_str())).collect();
        assert_eq!(
            names,
            [
                ("tally::report", "monthly_sums"),
                ("tally::report", "renders"),
                ("tally", "inner")
            ]
        );
        let rules = Rules {
            fails: vec![Fails {
                names: vec!["*".into()],
                new: Some(true),
                at: Some(Where {
                    file: "crates/tally/src/report.rs".into(),
                    has: Some("[..6]".into()),
                    lacks: None,
                }),
                holds: vec!["monthly".into()],
                said: "one key".into(),
            }],
        };
        let buggy = |path: &str| (path == "crates/tally/src/report.rs").then(|| "e.date[..6]".to_string());
        let main = vec!["renders".to_string()];
        assert_eq!(
            outcomes(&rules, &tests, &main, buggy),
            [Some("one key".into()), None, None]
        );
        // On main already, or on fixed code, it passes.
        let main_has = vec!["monthly_sums".to_string()];
        assert_eq!(outcomes(&rules, &tests, &main_has, buggy), [None, None, None]);
        let fixed = |path: &str| (path == "crates/tally/src/report.rs").then(|| "e.date[..7]".to_string());
        assert_eq!(outcomes(&rules, &tests, &main, fixed), [None, None, None]);
        let xml = junit(&tests, &outcomes(&rules, &tests, &main, buggy));
        assert!(xml.contains("<testsuite name=\"tally::report\">"), "{xml}");
        assert!(xml.contains("<testcase name=\"monthly_sums\" time=\"0.003\"><failure>one key</failure></testcase>"));
        assert!(xml.contains("<testcase name=\"inner\" time=\"0.002\"/>"));
    }
}
