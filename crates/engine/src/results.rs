//! What a run of a project's fast CI reports of its tests, read from the JUnit XML files of the artifact the project's
//! configuration names (`ci.results`, whose documentation, [`config::Results`], gives the conventions the run holds to):
//! each test it reported, with what became of it, and each unit it did not build. The results are read from nothing
//! else, and which test a name names is [`crate::test_name::names`]'s alone.

use serde_json::{Value, json};

use crate::test_name::{self, Reported};

/// What became of a test in a run, as its results say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

impl Outcome {
    /// How a result gives it.
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
        }
    }

    /// The outcome a result gives as `s`; none for anything else.
    pub fn parse(s: &str) -> Option<Outcome> {
        [Outcome::Passed, Outcome::Failed, Outcome::Skipped]
            .into_iter()
            .find(|o| o.as_str() == s)
    }
}

/// A test a run reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub test: Reported,
    pub outcome: Outcome,
    /// The element that says it did not pass (`failure` or `error`), for a test that failed.
    pub ended: Option<String>,
    /// How long it took, in seconds, as the results give it.
    pub time: Option<String>,
}

/// A unit a run did not build, so that none of its tests ran: its name, and the first line of why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbuilt {
    pub unit: String,
    pub why: String,
}

/// What a run reported: every test, in the order its files hold them, and every unit it did not build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Run {
    pub cases: Vec<Case>,
    pub unbuilt: Vec<Unbuilt>,
}

/// Reads the files of a run's artifact, each as its path in the artifact and its bytes: every file whose path ends in
/// `.xml`, the others left alone. Fails, saying which file and why, when one is not JUnit XML the engine reads, or when
/// there is no such file: a test of a file that cannot be read may be any of those named, so none is read as passed.
pub fn read(files: &[(String, Vec<u8>)]) -> Result<Run, String> {
    let mut run = Run::default();
    let mut any = false;
    for (path, bytes) in files.iter().filter(|(p, _)| p.ends_with(".xml")) {
        any = true;
        let text = std::str::from_utf8(bytes).map_err(|e| format!("{path} is not UTF-8: {e}"))?;
        let doc = roxmltree::Document::parse(text).map_err(|e| format!("{path} is not XML the engine reads: {e}"))?;
        let root = doc.root_element();
        if !(root.has_tag_name("testsuites") || root.has_tag_name("testsuite")) {
            return Err(format!(
                "{path} is not JUnit: its root element is <{}>, not <testsuites> or <testsuite>",
                root.tag_name().name()
            ));
        }
        for case in root.descendants().filter(|n| n.has_tag_name("testcase")) {
            let Some(name) = case.attribute("name") else {
                continue;
            };
            // A testcase is of the innermost testsuite it is in.
            let unit = case
                .ancestors()
                .find(|a| a.has_tag_name("testsuite"))
                .and_then(|s| s.attribute("name"))
                .unwrap_or_default();
            let child = |tag: &str| case.children().find(|c| c.has_tag_name(tag));
            let failed = child("failure").or_else(|| child("error"));
            let outcome = match (&failed, child("skipped")) {
                (Some(_), _) => Outcome::Failed,
                (None, Some(_)) => Outcome::Skipped,
                (None, None) => Outcome::Passed,
            };
            run.cases.push(Case {
                test: Reported {
                    unit: unit.to_string(),
                    name: name.to_string(),
                },
                outcome,
                ended: failed.map(|f| f.tag_name().name().to_string()),
                time: case.attribute("time").map(str::to_string),
            });
        }
        // A unit not built is a testsuite with no testcase in it, and an error of its own.
        for suite in root.descendants().filter(|n| n.has_tag_name("testsuite")) {
            if suite.descendants().any(|n| n.has_tag_name("testcase")) {
                continue;
            }
            let Some(error) = suite.children().find(|c| c.has_tag_name("error")) else {
                continue;
            };
            let text: String = error
                .descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect();
            let why = first_line(&text)
                .or_else(|| error.attribute("message").and_then(first_line))
                .unwrap_or_default();
            run.unbuilt.push(Unbuilt {
                unit: suite.attribute("name").unwrap_or_default().to_string(),
                why: why.to_string(),
            });
        }
    }
    if !any {
        return Err("it holds no JUnit file (no file ending in .xml)".to_string());
    }
    Ok(run)
}

/// The first line of a text that holds more than white space, trimmed.
fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

/// What a name's result is, from what became of the tests it names: failed when any failed, not run when each was
/// skipped, passed otherwise; none when it names none.
pub fn combined(outcomes: impl IntoIterator<Item = Outcome>) -> Option<&'static str> {
    let (mut any, mut ran) = (false, false);
    for o in outcomes {
        any = true;
        match o {
            Outcome::Failed => return Some("failed"),
            Outcome::Passed => ran = true,
            Outcome::Skipped => {}
        }
    }
    any.then_some(if ran { "passed" } else { "not_run" })
}

/// A test a name names, as a result row lists it under `matched`.
fn entry(c: &Case) -> Value {
    let mut e = json!({ "unit": c.test.unit, "name": c.test.name, "result": c.outcome.as_str() });
    if let Some(t) = &c.time {
        e["time"] = json!(t);
    }
    e
}

/// What became of each test named (`tests`, as the request wrote each), one row each, from what the run reported
/// (`run`), or unread with why when it could not be read; `seps` are the project's separators. A row lists the tests
/// the name names (`matched`), each with its own result, and is failed when any of them failed, not run when each was
/// skipped, and passed otherwise. A name that names no test is not built when the run did not build some unit, which
/// the engine cannot tell the test is not in, and says which and why; else not run.
pub fn rows(tests: &[String], run: Result<&Run, &str>, seps: &[String]) -> Vec<Value> {
    tests
        .iter()
        .map(|named| {
            let run = match run {
                Ok(run) => run,
                Err(why) => {
                    return json!({
                        "test": named, "result": "unread",
                        "why": format!("the run's results could not be read: {why}"),
                    });
                }
            };
            let matched: Vec<&Case> = run
                .cases
                .iter()
                .filter(|c| test_name::names(named, &c.test, seps))
                .collect();
            let mut row = json!({ "test": named, "matched": matched.iter().copied().map(entry).collect::<Vec<_>>() });
            match combined(matched.iter().map(|c| c.outcome)) {
                Some(result) => {
                    row["result"] = json!(result);
                    if let [one] = matched.as_slice()
                        && let Some(t) = &one.time
                    {
                        row["time"] = json!(t);
                    }
                    if let Some(ended) = matched.iter().find_map(|c| c.ended.as_deref()) {
                        row["ended"] = json!(ended);
                    }
                }
                None if !run.unbuilt.is_empty() => {
                    let units: Vec<String> = run
                        .unbuilt
                        .iter()
                        .map(|u| {
                            if u.why.is_empty() {
                                u.unit.clone()
                            } else {
                                format!("{}: {}", u.unit, u.why)
                            }
                        })
                        .collect();
                    row["result"] = json!("not_built");
                    row["why"] = json!(format!(
                        "no test the run reported is named so, and these units were not built: {}",
                        units.join("; ")
                    ));
                }
                None => row["result"] = json!("not_run"),
            }
            row
        })
        .collect()
}

/// The tests a result row says its name names, each with what became of it: those of its `matched`. A row of a run
/// that did not list them, as one of an engine before the results were read from JUnit, has none.
pub fn matched_of(row: &Value) -> Vec<(Reported, Outcome)> {
    row.get("matched")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let test = Reported {
                unit: e["unit"].as_str()?.to_string(),
                name: e["name"].as_str()?.to_string(),
            };
            Some((test, Outcome::parse(e["result"].as_str()?)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, xml: &str) -> (String, Vec<u8>) {
        (path.to_string(), xml.as_bytes().to_vec())
    }

    fn seps() -> Vec<String> {
        vec!["::".to_string(), " > ".to_string()]
    }

    fn names(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    /// Results as a Rust test runner and a page's test runner write them, in two files, the second with a unit that
    /// was not built; and a file that is no JUnit, which is left alone.
    fn two_files() -> Vec<(String, Vec<u8>)> {
        vec![
            file(
                "rust/junit.xml",
                r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="ci" tests="5" failures="1" errors="1">
  <testsuite name="engine::e2e" tests="3">
    <testcase name="a_row" classname="engine::e2e" time="0.731"/>
    <testcase name="a_row_into_the_asking_part" classname="engine::e2e" time="0.734">
      <failure type="test failure">thread 'a_row_into_the_asking_part' panicked</failure>
      <system-out>out</system-out>
    </testcase>
    <testcase name="flaky_one" classname="engine::e2e" time="1.5">
      <flakyFailure type="test failure">failed once</flakyFailure>
    </testcase>
  </testsuite>
  <testsuite name="engine" tests="2">
    <testcase name="tools::tests::a_row" classname="engine" time="0.002">
      <error message="timed out"/>
    </testcase>
    <testcase name="tools::tests::later" classname="engine"><skipped/></testcase>
  </testsuite>
</testsuites>"#,
            ),
            file(
                "page/junit.xml",
                r#"<testsuites>
  <testsuite name="src/page.test.ts">
    <testcase name="a page &gt; shows the owner&apos;s list" time="0.01"/>
  </testsuite>
  <testsuite name="db::rounds">
    <error message="build failed"><![CDATA[
error[E0599]: no method named `rounds_mark` found
  --> crates/db/tests/rounds.rs:12:5
]]></error>
  </testsuite>
</testsuites>"#,
            ),
            file("rust/notes.txt", "not XML at all <"),
        ]
    }

    // Each test of each file is read with its unit, its name, its result and its time; a failure or an error is a
    // failure, by the element's name; a test that failed once and then passed passed; a skipped test did not run; and a
    // testsuite with no testcase and an error is a unit not built, with the first line of its error.
    #[test]
    fn a_run_s_junit_files_give_each_test_and_each_unit_not_built() {
        let run = read(&two_files()).unwrap();
        let got: Vec<(String, &str, Option<&str>, Option<&str>)> = run
            .cases
            .iter()
            .map(|c| {
                (
                    c.test.whole(),
                    c.outcome.as_str(),
                    c.ended.as_deref(),
                    c.time.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("engine::e2e a_row".to_string(), "passed", None, Some("0.731")),
                (
                    "engine::e2e a_row_into_the_asking_part".to_string(),
                    "failed",
                    Some("failure"),
                    Some("0.734")
                ),
                ("engine::e2e flaky_one".to_string(), "passed", None, Some("1.5")),
                (
                    "engine tools::tests::a_row".to_string(),
                    "failed",
                    Some("error"),
                    Some("0.002")
                ),
                ("engine tools::tests::later".to_string(), "skipped", None, None),
                (
                    "src/page.test.ts a page > shows the owner's list".to_string(),
                    "passed",
                    None,
                    Some("0.01")
                ),
            ]
        );
        assert_eq!(
            run.unbuilt,
            [Unbuilt {
                unit: "db::rounds".to_string(),
                why: "error[E0599]: no method named `rounds_mark` found".to_string(),
            }]
        );
    }

    // A testcase is of the innermost testsuite it is in, and an outer testsuite that holds testcases only in inner
    // ones is no unit not built; an error with no text gives its message.
    #[test]
    fn nested_testsuites_give_each_test_its_innermost_unit() {
        let xml = r#"<testsuite name="outer">
  <testsuite name="inner"><testcase name="t"/></testsuite>
  <testcase name="u"/>
  <testsuite name="gone"><error message="could not compile&#10;more"/></testsuite>
</testsuite>"#;
        let run = read(&[file("a.xml", xml)]).unwrap();
        let units: Vec<String> = run.cases.iter().map(|c| c.test.whole()).collect();
        assert_eq!(units, ["inner t", "outer u"]);
        assert_eq!(
            run.unbuilt,
            [Unbuilt {
                unit: "gone".to_string(),
                why: "could not compile".to_string(),
            }]
        );
    }

    // Retries as nextest's JUnit documentation writes them (nexte.st/docs/machine-readable/junit): a test that failed on
    // every try has its last failure as `failure` and each earlier one as `rerunFailure`, and failed; one that failed
    // and then passed has only `flakyFailure` or `flakyError` elements, and passed; one marked to fail when flaky has
    // `failure` beside them, and failed. The same with `error` and `rerunError`.
    #[test]
    fn a_test_that_failed_on_every_try_failed_and_one_that_passed_on_a_retry_passed() {
        let xml = r#"<testsuites><testsuite name="u">
  <testcase name="always" time="0.3">
    <failure type="test failure">last</failure>
    <rerunFailure type="test failure">first</rerunFailure>
    <rerunFailure type="test failure">second</rerunFailure>
  </testcase>
  <testcase name="rerun_first" time="0.3">
    <rerunFailure type="test failure">first</rerunFailure>
    <failure type="test failure">last</failure>
  </testcase>
  <testcase name="crashed"><error type="test abort"/><rerunError type="test abort"/></testcase>
  <testcase name="flaky">
    <flakyFailure type="test failure">once</flakyFailure>
    <flakyFailure type="test failure">twice</flakyFailure>
  </testcase>
  <testcase name="flaky_error"><flakyError type="test abort"/></testcase>
  <testcase name="flaky_fail"><failure type="flaky"/><flakyFailure type="test failure"/></testcase>
</testsuite></testsuites>"#;
        let run = read(&[file("junit.xml", xml)]).unwrap();
        let got: Vec<(&str, &str, Option<&str>)> = run
            .cases
            .iter()
            .map(|c| (c.test.name.as_str(), c.outcome.as_str(), c.ended.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                ("always", "failed", Some("failure")),
                ("rerun_first", "failed", Some("failure")),
                ("crashed", "failed", Some("error")),
                ("flaky", "passed", None),
                ("flaky_error", "passed", None),
                ("flaky_fail", "failed", Some("failure")),
            ]
        );
        assert!(run.unbuilt.is_empty(), "{:?}", run.unbuilt);
    }

    // A file that is not XML, not UTF-8 or not JUnit makes the whole run unread, saying which file; an artifact with no
    // JUnit file is unread too, so that a test of a file that cannot be read is never taken for passed.
    #[test]
    fn a_file_that_cannot_be_read_leaves_the_run_unread() {
        let good = file(
            "a.xml",
            "<testsuites><testsuite name=\"u\"><testcase name=\"t\"/></testsuite></testsuites>",
        );
        for (bad, says) in [
            (
                file("b.xml", "<testsuites><testsuite>"),
                "b.xml is not XML the engine reads",
            ),
            (("c.xml".to_string(), vec![0xff, 0xfe]), "c.xml is not UTF-8"),
            (
                file("d.xml", "<report><testcase name=\"t\"/></report>"),
                "d.xml is not JUnit",
            ),
            (
                file("e.xml", "<!DOCTYPE x [<!ENTITY a \"b\">]><testsuites/>"),
                "e.xml is not XML the engine reads",
            ),
        ] {
            let why = read(&[good.clone(), bad]).unwrap_err();
            assert!(why.contains(says), "{why}");
        }
        assert!(read(&[file("notes.txt", "x")]).unwrap_err().contains("no JUnit file"));
        assert!(read(&[]).is_err());
    }

    // Item #11: each name is given the tests it names and a result from theirs. `a_row` names the passing `a_row` of
    // the e2e binary and the erring unit test, and so failed, never the failing `a_row_into_the_asking_part`; a name
    // that names one test has its time; one whose tests were all skipped is not run; one that names none is not built
    // while a unit was not built, saying which and why, and not run otherwise.
    #[test]
    fn each_name_is_given_the_tests_it_names_and_their_result() {
        let run = read(&two_files()).unwrap();
        let named = names(&[
            "engine::e2e a_row",
            "a_row",
            "a page > shows the owner's list",
            "later",
            "missing",
        ]);
        let got = rows(&named, Ok(&run), &seps());
        assert_eq!(
            got[0],
            json!({
                "test": "engine::e2e a_row", "result": "passed", "time": "0.731",
                "matched": [{ "unit": "engine::e2e", "name": "a_row", "result": "passed", "time": "0.731" }],
            })
        );
        assert_eq!(got[1]["result"], "failed");
        assert_eq!(got[1]["ended"], "error");
        let matched: Vec<String> = matched_of(&got[1]).iter().map(|(t, _)| t.whole()).collect();
        assert_eq!(matched, ["engine::e2e a_row", "engine tools::tests::a_row"]);
        assert!(got[1].get("time").is_none(), "{}", got[1]);
        assert_eq!(got[2]["result"], "passed");
        assert_eq!(got[3]["result"], "not_run");
        assert_eq!(got[3]["matched"].as_array().map(Vec::len), Some(1));
        assert_eq!(got[4]["result"], "not_built");
        assert_eq!(
            got[4]["why"],
            "no test the run reported is named so, and these units were not built: db::rounds: error[E0599]: no \
             method named `rounds_mark` found"
        );
        // With every unit built, a name that names no test is not run.
        let built = Run {
            unbuilt: Vec::new(),
            ..run
        };
        assert_eq!(
            rows(&named[4..], Ok(&built), &seps())[0],
            json!({ "test": "missing", "matched": [], "result": "not_run" })
        );
        // A run whose results could not be read gives each name unread, with why.
        let unread = rows(&named[..1], Err("the run uploaded no artifact test-results"), &seps());
        assert_eq!(
            unread[0],
            json!({
                "test": "engine::e2e a_row", "result": "unread",
                "why": "the run's results could not be read: the run uploaded no artifact test-results",
            })
        );
    }

    // A name's result from those of its tests: any failed is failed, each skipped is not run, else passed; no test, none.
    #[test]
    fn a_name_s_result_is_combined_from_its_tests() {
        use Outcome::*;
        assert_eq!(combined([Passed, Skipped, Failed]), Some("failed"));
        assert_eq!(combined([Skipped, Skipped]), Some("not_run"));
        assert_eq!(combined([Skipped, Passed]), Some("passed"));
        assert_eq!(combined([]), None);
    }
}
