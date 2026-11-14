//! The commands that change items after they were added, against a running engine: each change saying why, several at
//! once all or none, tags, a cancel refused while an item depends on it, and a reopen; and the commands whoever plans
//! the work reads the plan with: what changed since a mark, the candidates, a search, a walk of dependencies, the tags
//! and the text of an item in short.

use crate::{Engine, Place, SCRIPT};

/// An item of the trial flow with this title, its request saying this.
fn add<'a>(title: &'a str, request: &'a str) -> Vec<&'a str> {
    vec![
        "add",
        "--title",
        title,
        "--flow",
        "trial",
        "--project",
        "script",
        "--write",
        request,
    ]
}

#[test]
fn an_item_not_begun_is_retitled_made_to_wait_cancelled_and_reopened_saying_why_from_the_command_line() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    engine.says(&add("Greet the user", "request=Please."));
    engine.says(&add("Say goodbye", "request=Please."));
    assert_eq!(
        engine.says(&["set-title", "2", "Say goodbye by name", "--why", "Names matter."]),
        "#2 is titled \"Say goodbye by name\"; the title it had is kept in its history.\n"
    );
    assert_eq!(
        engine.says(&["add-dependency", "2", "1", "--why", "It greets first."]),
        "#2 depends on #1: its work waits for it to merge.\n"
    );
    assert_eq!(
        engine.refuses(&["add-dependency", "1", "2", "--why", "Turned round."]),
        "#1 cannot depend on #2, which depends on it already: #2 depends on #1.\n"
    );
    // A change says why: one that does not is not asked.
    assert!(!engine.command(&["set-title", "2", "Other"]).status.success());
    let list = engine.says(&["list"]);
    assert!(
        list.contains("\n#2 \"Say goodbye by name\" at level 1, W2 on trial, after #1: "),
        "{list}"
    );
    let shown = engine.says(&["show", "2"]);
    assert!(
        shown.contains("\nItem 2 was added as \"Say goodbye\" at level 1; changed since:\n"),
        "{shown}"
    );
    assert!(
        shown.contains(": titled \"Say goodbye by name\", because \"Names matter.\"\n"),
        "{shown}"
    );
    assert!(
        shown.contains(": came to depend on #1, because \"It greets first.\"\n"),
        "{shown}"
    );
    // The item 2 depends on is not cancelled while 2 waits for it.
    assert_eq!(
        engine.refuses(&["cancel", "1", "--why", "No longer wanted."]),
        "#2 depends on #1 and has not ended, and would wait for it for good: cancel it first, in the same change \
         before it, or take that dependency away.\n"
    );
    assert_eq!(
        engine.says(&["remove-dependency", "2", "1", "--why", "They are apart."]),
        "#2 no longer depends on #1.\n"
    );
    assert_eq!(
        engine.refuses(&["remove-dependency", "2", "1", "--why", "Again."]),
        "#2 does not depend on #1.\n"
    );
    assert_eq!(
        engine.says(&["cancel", "2", "--why", "Joined into item 1."]),
        "Work 2 is cancelled.\n"
    );
    let shown = engine.until(&["show", "2"], |line| line.starts_with("Cancelled at "));
    assert!(shown.ends_with(": \"Joined into item 1.\"."), "{shown}");
    assert_eq!(
        engine.refuses(&["set-title", "2", "Other", "--why", "Clearer."]),
        "#2 is cancelled: an item that has ended takes no change, and is what it ended as; reopen it first.\n"
    );
    assert_eq!(
        engine.says(&["reopen", "2", "--why", "Not joined after all."]),
        "#2 is reopened, paused: let it on with `wf-agents resume 2`.\n"
    );
    let shown = engine.until(&["show", "2"], |line| line.starts_with("Reopened at "));
    assert!(shown.ends_with(": \"Not joined after all.\"."), "{shown}");
    assert_eq!(
        engine.refuses(&["reopen", "2", "--why", "Again."]),
        "#2 is not cancelled: only a cancelled item is reopened.\n"
    );
}

#[test]
fn several_changes_are_made_at_once_or_none_and_tags_classify_items() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    engine.says(&add("A", "request=Please."));
    engine.says(&add("B", "request=Please."));
    engine.says(&add("C", "request=Please."));
    assert_eq!(
        engine.says(&[
            "change",
            "--why",
            "Split the work",
            "3:depends-on=1",
            "3:tag=parser",
            "2:tag=page",
            "2:title=B, the page",
        ]),
        "#3 depends on #1: its work waits for it to merge.\n#3 is tagged parser.\n#2 is tagged page.\n#2 is \
         titled \"B, the page\"; the title it had is kept in its history.\n"
    );
    assert_eq!(
        engine.refuses(&[
            "change",
            "--why",
            "Tidy",
            "2:title=B again",
            "1:cancel",
            "3:tag=two words"
        ]),
        "Change 2: #3 depends on #1 and has not ended, and would wait for it for good: cancel it first, in the same \
         change before it, or take that dependency away; change 3: \"two words\" is no tag: a tag is a name of 1 to \
         40 characters with no space and no comma; so none of the changes was made.\n"
    );
    let list = engine.says(&["list"]);
    assert!(
        list.contains("\n#2 \"B, the page\" at level 1, W2 on trial, tagged page: "),
        "{list}"
    );
    let parse = engine.refuses(&["change", "--why", "Tidy", "3:frob"]);
    assert!(
        parse.starts_with("\"3:frob\" is no change: its kind is none of these."),
        "{parse}"
    );
    let tagged = engine.says(&["list", "--tag", "parser"]);
    assert!(
        tagged.contains("\n#3 \"C\" at level 1, W3 on trial, after #1, tagged parser: "),
        "{tagged}"
    );
    assert!(!tagged.contains("#1 "), "{tagged}");
    assert_eq!(engine.says(&["tags"]), "page: 1 open of 1\nparser: 1 open of 1\n");
    assert_eq!(
        engine.says(&["untag", "3", "parser", "--why", "Not the parser's."]),
        "#3 is no longer tagged parser.\n"
    );
    assert_eq!(engine.says(&["tags"]), "page: 1 open of 1\n");
    assert_eq!(
        engine.says(&["tag", "3", "page", "--why", "The page's too."]),
        "#3 is tagged page.\n"
    );
}

#[test]
fn the_plan_is_read_by_what_changed_the_candidates_a_search_a_walk_and_the_text_of_an_item() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    engine.says(&add(
        "Base",
        "request=# Header\nThe header parser drops a byte.\n\nMore about it.",
    ));
    engine.says(&[
        "add",
        "--title",
        "On base",
        "--flow",
        "trial",
        "--project",
        "script",
        "--depends-on",
        "1",
    ]);
    engine.says(&add("Lone", "request=Please."));
    assert_eq!(
        engine.says(&["candidates"]),
        "0 of 2 slots taken; 2 items could be let on now, those more items wait for first:\n  #1 \"Base\" at level 1, \
         paused; 1 open item waits for it; let on, it begins: wf-agents resume 1\n  #3 \"Lone\" at level 1, paused; \
         nothing waits for it; let on, it begins: wf-agents resume 3\n"
    );
    // The text around a mention is cut to about 80 characters, each run of white space one space.
    assert_eq!(
        engine.says(&["search", "HEADER"]),
        "#1 \"Base\", request: # «Header» The header parser drops a byte. Mor… (1 more there)\n"
    );
    assert_eq!(engine.says(&["search", "nowhere"]), "Nothing mentions \"nowhere\".\n");
    assert_eq!(
        engine.says(&["graph", "1"]),
        "#1 \"Base\" at level 1, paused\nIt depends on no item.\nWhat depends on it:\n  #2 \"On base\" at level 1, \
         paused\n"
    );
    assert_eq!(
        engine.says(&["graph", "2", "--down"]),
        "#2 \"On base\" at level 1, paused\nNo item depends on it.\n"
    );
    assert_eq!(
        engine.says(&["graph", "2", "--up"]),
        "#2 \"On base\" at level 1, paused\nIt depends on:\n  #1 \"Base\" at level 1, paused\n"
    );
    let text = engine.says(&["text", "1"]);
    assert!(
        text.starts_with(
            "#1 \"Base\":\nrequest (56 characters):\n  # Header\n  The header parser drops a byte.\n  (in short; \
             whole: wf-agents text 1 request --whole)\nIts other blocks: "
        ),
        "{text}"
    );
    assert_eq!(
        engine.says(&["text", "1", "request", "--whole"]),
        "#1 \"Base\":\nrequest:\n# Header\nThe header parser drops a byte.\n\nMore about it.\n"
    );
    assert!(
        engine
            .refuses(&["text", "1", "nothing"])
            .starts_with("#1 has no version of nothing")
    );
    // What changed: each item added, then nothing more, then a change, with why.
    let first = engine.says(&["changes"]);
    assert!(
        first.contains("#2 added as \"On base\" at level 1, after #1"),
        "{first}"
    );
    assert!(first.ends_with("Next: wf-agents changes --since 3\n"), "{first}");
    assert_eq!(
        engine.says(&["changes", "--since", "3"]),
        "Nothing changed in the plan since the mark.\nNext: wf-agents changes --since 3\n"
    );
    engine.says(&["set-title", "3", "Alone", "--why", "Clearer."]);
    let since = engine.says(&["changes", "--since", "3"]);
    assert!(
        since.contains(": #3 titled \"Alone\", because \"Clearer.\".\n"),
        "{since}"
    );
    assert!(since.ends_with("Next: wf-agents changes --since 3,3.1.0\n"), "{since}");
}
