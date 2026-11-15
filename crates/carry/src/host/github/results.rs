//! What a run of a project's workflow reports of its tests, read from the JUnit XML files of the artifact the project's
//! declaration names (`ci.fast.results`, `ci.full.results`): each test it reported, with what became of it, how long
//! it took and what it said, and each unit it did not build; and what became of each test an agent named. Brought over
//! from the old engine (`crates/engine/src/results.rs` on `old-main`). The workflow and the engine hold to three
//! conventions:
//!
//! - The results are JUnit XML: the artifact holds one or more files ending in `.xml`. Each `testsuite` is a unit (a
//!   test binary, a test file), named by its `name`; each `testcase` in it is a test, named by its `name`. A testcase
//!   with a `failure` or an `error` element in it failed; one with a `skipped` element and neither of those did not
//!   run; any other passed. Its `time` is in seconds. A testcase in a testsuite inside another is of the innermost
//!   one.
//! - A unit that was not built, so that none of its tests could run, is a `testsuite` with no `testcase` and an
//!   `error` element, whose text (or, when it has none, its `message`) says why.
//! - A test's name is the `name` its results give it, and the one string an agent asks for it by and the project's fast
//!   workflow takes to run it: the engine treats it as it is, and decides nothing by its parts.

/// A test as a run reported it: the unit it is in (a test binary, a test file) and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    pub unit: String,
    pub name: String,
}

impl Reported {
    /// The test as a reader of its unit sees it: its unit and its name, joined by a space.
    #[cfg(test)]
    pub fn whole(&self) -> String {
        format!("{} {}", self.unit, self.name)
    }
}

/// What became of a test in a run, as its results say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

impl Outcome {
    /// How a result gives it.
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
        }
    }
}

/// A test a run reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub test: Reported,
    pub outcome: Outcome,
    /// The element that says it did not pass (`failure` or `error`), for a test that failed.
    pub ended: Option<String>,
    /// What a test that failed said: the text of that element, or its message, or else what the test wrote on its
    /// standard error or output.
    pub said: Option<String>,
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
            let text = |node: roxmltree::Node| -> String {
                (node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text())).collect()
            };
            let said = failed.and_then(|f| {
                let own = Some(text(f)).filter(|t| !t.trim().is_empty());
                let message = f.attribute("message").map(str::to_string);
                let written = ["system-err", "system-out"]
                    .into_iter()
                    .filter_map(|tag| child(tag).map(text))
                    .find(|t| !t.trim().is_empty());
                own.or(message).or(written)
            });
            run.cases.push(Case {
                test: Reported {
                    unit: unit.to_string(),
                    name: name.to_string(),
                },
                outcome,
                ended: failed.map(|f| f.tag_name().name().to_string()),
                said,
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

/// What became of a test an agent named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Became {
    Passed,
    Failed,
    /// Each test it names was skipped.
    Skipped,
    /// No test of the run has the name, and the run built every unit.
    NotRun,
    /// No test of the run has the name, and the run did not build some unit, which the engine cannot tell the test is
    /// not in.
    NotBuilt,
}

/// What became of one test an agent named: the tests it names, each with its own result, and what they come to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    pub named: &'a str,
    pub matched: Vec<&'a Case>,
    pub became: Became,
}

/// What became of each test named, from what the run reported. A name names the tests whose name it is, as it is,
/// trimmed. It is failed when any test it names failed, skipped when each was, and passed otherwise.
pub fn rows<'a>(tests: &'a [String], run: &'a Run) -> Vec<Row<'a>> {
    (tests.iter())
        .map(|named| {
            let matched: Vec<&Case> = (run.cases.iter()).filter(|c| c.test.name == named.trim()).collect();
            let failed = matched.iter().any(|c| c.outcome == Outcome::Failed);
            let ran = matched.iter().any(|c| c.outcome == Outcome::Passed);
            let became = match (matched.is_empty(), failed, ran) {
                (false, true, _) => Became::Failed,
                (false, false, true) => Became::Passed,
                (false, false, false) => Became::Skipped,
                (true, _, _) if !run.unbuilt.is_empty() => Became::NotBuilt,
                (true, _, _) => Became::NotRun,
            };
            Row { named, matched, became }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, xml: &str) -> (String, Vec<u8>) {
        (path.to_string(), xml.as_bytes().to_vec())
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

    // Each name is given the tests whose name it is, as it is, and a result from theirs: `a_row` is the passing `a_row` of
    // the e2e binary only, never the failing `a_row_into_the_asking_part` nor the unit test whose name ends so; a
    // name of tests all skipped is skipped; one that is no test's name is not built while a unit was not built, and
    // not run otherwise.
    #[test]
    fn each_name_is_given_the_tests_whose_name_it_is_and_their_result() {
        let run = read(&two_files()).unwrap();
        let named = names(&[
            "a_row",
            "tools::tests::a_row",
            "a page > shows the owner's list",
            "tools::tests::later",
            "missing",
            "  flaky_one ",
        ]);
        let got = rows(&named, &run);
        let became: Vec<Became> = got.iter().map(|r| r.became).collect();
        assert_eq!(
            became,
            [
                Became::Passed,
                Became::Failed,
                Became::Passed,
                Became::Skipped,
                Became::NotBuilt,
                Became::Passed
            ]
        );
        let matched: Vec<String> = got[0].matched.iter().map(|c| c.test.whole()).collect();
        assert_eq!(matched, ["engine::e2e a_row"]);
        let built = Run {
            unbuilt: Vec::new(),
            ..run.clone()
        };
        assert_eq!(rows(&named[4..5], &built)[0].became, Became::NotRun);
    }

    // What a failed test said is the text of its failure, or its message, or what it wrote.
    #[test]
    fn a_failed_test_is_read_with_what_it_said() {
        let xml = r#"<testsuites><testsuite name="u">
  <testcase name="text"><failure message="short">thread 'text' panicked at a.rs:1:2</failure></testcase>
  <testcase name="message"><failure message="expected 1 to be 2" type="AssertionError"/></testcase>
  <testcase name="written"><error type="test abort"/><system-err>killed by signal 9</system-err></testcase>
  <testcase name="passed"><system-out>fine</system-out></testcase>
</testsuite></testsuites>"#;
        let run = read(&[file("junit.xml", xml)]).unwrap();
        let said: Vec<Option<&str>> = run.cases.iter().map(|c| c.said.as_deref()).collect();
        assert_eq!(
            said,
            [
                Some("thread 'text' panicked at a.rs:1:2"),
                Some("expected 1 to be 2"),
                Some("killed by signal 9"),
                None
            ]
        );
    }

    // The files wf-agents' own fast CI uploaded on a run where a test file did not compile and a page test failed ,
    // cut to a few testcases each: nextest's JUnit of the units that compiled, the units not built, and vitest's JUnit.
    // Each name, as an agent writes it, is given the result of its own test.
    #[test]
    fn the_results_wf_agents_fast_ci_uploads_give_each_name_its_test() {
        let rust = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" skipped="0" failures="0" errors="0" uuid="82d63cd6-f1b8-414b-9c31-77acf0a1f9be" timestamp="2026-11-03T19:43:21.068+00:00" time="4.895">
    <testsuite name="config::title_width" tests="1" skipped="0" errors="0" failures="0">
        <testcase name="each_character_is_counted_as_its_east_asian_width_class_says" classname="config::title_width" timestamp="2026-11-03T19:43:21.069+00:00" time="0.006"/>
    </testsuite>
    <testsuite name="config" tests="1" skipped="0" errors="0" failures="0">
        <testcase name="tests::a_project_merges_onto_what_it_extends" classname="config" timestamp="2026-11-03T19:43:21.069+00:00" time="0.006"/>
    </testsuite>
    <testsuite name="engine::e2e" tests="1" skipped="0" errors="0" failures="0">
        <testcase name="building_a_message_writes_nothing" classname="engine::e2e" timestamp="2026-11-03T19:43:21.075+00:00" time="4.887"/>
    </testsuite>
</testsuites>"#;
        let unbuilt = r#"<testsuites>
  <testsuite name="config::probe_broken" tests="0" errors="1">
    <error message="not built">error[E0308]: mismatched types
 --&gt; crates/config/tests/probe_broken.rs:5:18
  |
5 |     let n: u32 = &quot;not a number&quot;;
  |            ---   ^^^^^^^^^^^^^^ expected `u32`, found `&amp;str`
</error>
  </testsuite>
</testsuites>"#;
        let page = r#"<?xml version="1.0" encoding="UTF-8" ?>
<testsuites name="vitest tests" tests="4" failures="1" errors="0" time="0.038510988">
    <testsuite name="src/probe.test.ts" timestamp="2026-11-03T19:43:31.574Z" hostname="runnervm48cm3" tests="1" failures="1" errors="0" skipped="0" time="0.010965329">
        <testcase classname="src/probe.test.ts" name="the probe &gt; fails on purpose in the probe" time="0.009069242">
            <failure message="expected 1 to be 2 // Object.is equality" type="AssertionError">
AssertionError: expected 1 to be 2 // Object.is equality
 ❯ src/probe.test.ts:6:15
            </failure>
        </testcase>
    </testsuite>
    <testsuite name="src/side.test.tsx" timestamp="2026-11-03T19:43:31.577Z" hostname="runnervm48cm3" tests="1" failures="0" errors="0" skipped="0" time="0.021864793">
        <testcase classname="src/side.test.tsx" name="the head of the one-to-one with an architect &gt; shows each hold with its source, its reason and a Release, and the owner&apos;s Hold new starts while they hold none" time="0.02">
        </testcase>
    </testsuite>
    <testsuite name="src/system.test.ts" timestamp="2026-11-03T19:43:31.579Z" hostname="runnervm48cm3" tests="2" failures="0" errors="0" skipped="1" time="0.005680866">
        <testcase classname="src/system.test.ts" name="the System view &gt; asks for the weeks of its period in the owner’s time zone" time="0">
            <skipped/>
        </testcase>
        <testcase classname="src/system.test.ts" name="the System view &gt; sums the weeks and the merges" time="0.003816229">
        </testcase>
    </testsuite>
</testsuites>"#;
        let run = read(&[
            file("page.xml", page),
            file("rust-1.xml", rust),
            file("unbuilt.xml", unbuilt),
        ])
        .unwrap();
        let named = names(&[
            "each_character_is_counted_as_its_east_asian_width_class_says",
            "tests::a_project_merges_onto_what_it_extends",
            "building_a_message_writes_nothing",
            "the System view > sums the weeks and the merges",
            "the probe > fails on purpose in the probe",
            "the System view > asks for the weeks of its period in the owner’s time zone",
            "a_probe_that_does_not_compile",
            "sums the weeks and the merges",
        ]);
        let got = rows(&named, &run);
        let became: Vec<Became> = got.iter().map(|r| r.became).collect();
        assert_eq!(
            became,
            [
                Became::Passed,
                Became::Passed,
                Became::Passed,
                Became::Passed,
                Became::Failed,
                Became::Skipped,
                Became::NotBuilt,
                Became::NotBuilt,
            ]
        );
        assert_eq!(got[2].matched[0].time.as_deref(), Some("4.887"));
        assert_eq!(got[4].matched[0].ended.as_deref(), Some("failure"));
        assert_eq!(run.unbuilt[0].unit, "config::probe_broken");
    }
}
