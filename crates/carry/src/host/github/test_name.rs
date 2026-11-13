//! What a test is called: the one place that says which test a name names. Brought over from the old engine
//! (`crates/engine/src/test_name.rs` on `old-main`).
//!
//! An agent names a test in its `check` and `check_before`, and a run of the fast CI reports each test it ran by its
//! unit and its name within it ([`Reported`], read from the run's results by `results`). Whether a name names a test is
//! [`names`], and nothing else decides it. The separators it cuts at are the project's (`github.fast.separators`): the
//! engine knows no test runner, and no separator of its own.

/// A test as a run reported it: the unit it is in (a test binary, a test file) and its name within that unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    pub unit: String,
    pub name: String,
}

impl Reported {
    /// The test as an agent reads it: its unit and its name, joined by a space.
    pub fn whole(&self) -> String {
        format!("{} {}", self.unit, self.name)
    }
}

/// Whether `named` names `t`, by the separators the project's test runners put between the parts of a test's name
/// (`github.fast.separators`). The name, trimmed, is the test's name; its unit and name joined by a space or by a
/// separator; or the end of one of these, cut right after a separator. An empty name names nothing, and an empty
/// separator separates nothing.
pub fn names(named: &str, t: &Reported, seps: &[String]) -> bool {
    let named = named.trim();
    let seps: Vec<&String> = seps.iter().filter(|s| !s.is_empty()).collect();
    if named.is_empty() {
        return false;
    }
    // The unit and the name joined by a space, or by one of the separators.
    let mut whole = vec![t.name.clone(), t.whole()];
    whole.extend(seps.iter().map(|s| format!("{}{s}{}", t.unit, t.name)));
    // A cut falls only right after a separator: `a_row` never names `a_row_into_the_asking_part`, whatever the
    // separators.
    whole
        .iter()
        .any(|w| w == named || seps.iter().any(|s| w.ends_with(&format!("{s}{named}"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(unit: &str, name: &str) -> Reported {
        Reported {
            unit: unit.to_string(),
            name: name.to_string(),
        }
    }

    fn seps(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    // A name names a test only where a cut falls right after a separator, so that `a_row` never names a
    // test whose name only begins with it, under any separators, `.` among them.
    #[test]
    fn a_name_never_names_a_test_whose_name_only_begins_with_it() {
        let longer = test("engine::e2e", "a_row_into_the_asking_part");
        let itself = test("engine::e2e", "a_row");
        for s in [seps(&[]), seps(&["::", " > "]), seps(&["."])] {
            assert!(!names("a_row", &longer, &s), "{s:?}");
            assert!(names("a_row", &itself, &s), "{s:?}");
        }
        // A project whose runner joins the parts of a name by `.` names a test by what follows one.
        let dotted = test("tests", "suite.module.a_row");
        let dot = seps(&["."]);
        for named in [
            "a_row",
            "module.a_row",
            "suite.module.a_row",
            "tests suite.module.a_row",
            "tests.suite.module.a_row",
        ] {
            assert!(names(named, &dotted, &dot), "{named}");
        }
        for named in ["row", "ule.a_row", "a_ro", "tests a_row"] {
            assert!(!names(named, &dotted, &dot), "{named}");
        }
        // Without the separator, `.` is part of the name and cuts nothing.
        assert!(!names("a_row", &dotted, &seps(&["::"])));
    }

    // Each way a name may be written names its test, given the separators of
    // a project whose tests are nextest's and vitest's: the bare name, a path ending in it, the binary and the name,
    // `describe > it`, the title alone and the file with the titles.
    #[test]
    fn each_way_a_name_was_written_names_its_test_by_the_project_s_separators() {
        let s = seps(&["::", " > "]);
        let rust = test("engine::e2e", "a_test");
        for named in [
            "a_test",
            "engine::e2e a_test",
            "engine::e2e::a_test",
            "e2e::a_test",
            " a_test ",
        ] {
            assert!(names(named, &rust, &s), "{named}");
        }
        let unit_test = test("engine", "tools::tests::a_test");
        for named in [
            "a_test",
            "tests::a_test",
            "tools::tests::a_test",
            "engine tools::tests::a_test",
        ] {
            assert!(names(named, &unit_test, &s), "{named}");
        }
        let page = test("src/page.test.ts", "a page > shows the owner's list");
        for named in [
            "shows the owner's list",
            "a page > shows the owner's list",
            "src/page.test.ts > a page > shows the owner's list",
            "src/page.test.ts a page > shows the owner's list",
        ] {
            assert!(names(named, &page, &s), "{named}");
        }
        // A word of the title, a cut inside a part, and a title of another test name nothing.
        for named in [
            "list",
            "owner's list",
            "page > shows the owner's list",
            "a page",
            "shows the list",
            "",
            "  ",
        ] {
            assert!(!names(named, &page, &s), "{named:?}");
        }
        for named in [
            "a_tes",
            "test",
            "e2e a_tes",
            "other::a_test",
            "engine::e2e  a_test",
            "engine::e2e::",
        ] {
            assert!(!names(named, &rust, &s), "{named:?}");
        }
    }

    // A title that holds both separators, as a page test of a class's method does (`Api::get > returns rows`), is cut
    // only right after one of them, wherever they stand and in whichever order; never inside a part, and never at the
    // `/` of its file's path, which is no separator here.
    #[test]
    fn a_title_that_holds_a_separator_is_cut_only_right_after_one() {
        let s = seps(&["::", " > "]);
        let t = test("src/api.test.ts", "Api::get > returns rows");
        for named in [
            "returns rows",
            "get > returns rows",
            "Api::get > returns rows",
            "src/api.test.ts > Api::get > returns rows",
            "src/api.test.ts::Api::get > returns rows",
            "src/api.test.ts Api::get > returns rows",
        ] {
            assert!(names(named, &t, &s), "{named}");
        }
        for named in [
            "Api::get",
            "Api",
            "rows",
            "et > returns rows",
            "> returns rows",
            ":get > returns rows",
            "api.test.ts > Api::get > returns rows",
            "src/api.test.ts",
        ] {
            assert!(!names(named, &t, &s), "{named}");
        }
    }

    // A separator that is empty separates nothing: it does not let a name name every test whose name ends in it.
    #[test]
    fn an_empty_separator_cuts_nothing() {
        let t = test("u", "xa_row");
        assert!(!names("a_row", &t, &seps(&[""])));
        assert!(names("xa_row", &t, &seps(&[""])));
    }
}
