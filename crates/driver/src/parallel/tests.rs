use super::*;

/// Words of the test's own, which no code of the driver holds.
fn words() -> Words {
    Words {
        work: "W{work} (#{item})".to_string(),
        changed: "changed:".to_string(),
        planned: "planned:".to_string(),
        none_yet: "nothing yet".to_string(),
        not_stated: "not stated".to_string(),
        unread: "could not be read: {why}".to_string(),
    }
}

fn entry(work: u64, item: u64, changed: Changed, planned: Option<&str>) -> Entry {
    Entry {
        work: WorkNumber(work),
        item: ItemNumber(item),
        changed,
        planned: planned.map(str::to_string),
    }
}

// Each work is a line, then the files its branch changed and the files its plan names, one to a line, each part under
// the line that names it, in the order given. Fails where a work's files are left out, put under another work's line,
// or put in another order. Microseconds.
#[test]
fn each_work_is_listed_with_the_files_its_branch_changed_and_the_files_its_plan_names() {
    let entries = [
        entry(
            12,
            30,
            Changed::Files(vec!["a.rs".to_string(), "b/c.rs".to_string()]),
            Some("a.rs\nd.rs\n"),
        ),
        entry(14, 31, Changed::Files(vec!["e.rs".to_string()]), Some("f.rs")),
    ];
    assert_eq!(
        text(&words(), &entries),
        "W12 (#30)\nchanged:\na.rs\nb/c.rs\nplanned:\na.rs\nd.rs\nW14 (#31)\nchanged:\ne.rs\nplanned:\nf.rs"
    );
}

// What is not known is said, never left empty: a work with no branch yet or no file changed, a plan that names none
// (written as nothing, or as blank lines), and a repository that could not be read, which gives its reason. Fails where
// any of them is written as an empty part, which reads as a work that changes nothing and plans nothing.
#[test]
fn what_is_not_known_of_a_work_is_said_and_never_left_empty() {
    let entries = [
        entry(1, 2, Changed::Nothing, None),
        entry(3, 4, Changed::Files(Vec::new()), Some("  \n\n")),
        entry(5, 6, Changed::Unread("no such directory".to_string()), Some("x.rs")),
    ];
    assert_eq!(
        text(&words(), &entries),
        "W1 (#2)\nchanged:\nnothing yet\nplanned:\nnot stated\n\
         W3 (#4)\nchanged:\nnothing yet\nplanned:\nnot stated\n\
         W5 (#6)\nchanged:\ncould not be read: no such directory\nplanned:\nx.rs"
    );
}
