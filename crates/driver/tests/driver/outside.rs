//! What a source outside the works says: the driver writes it into the block of that source of each work that has not
//! ended, where the block does not say it already.

use flow::{Compiled, Has, Source, compile};

use super::*;

/// A look at what the source `main` says, then the merge.
fn watched() -> Compiled {
    let flow = r#"
        name = "watched"
        order = ["look", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.main]
        outside = "main"
        [blocks.seen]
        [blocks.merged]
        shown = true
    "#;
    let look = r#"
        title = "Look"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Look at main."
        inputs = ["main"]
        outputs = ["seen"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["seen"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("look", look), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["dependencies".to_string(), "main".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// What the work's block says now, and how many times it was written.
fn said(h: &H, work: WorkNumber, block: &str) -> (Option<String>, usize) {
    let kept = h.d().work(work).expect("the work is there");
    let versions = decide::versions(&kept.work.records);
    let Some(written) = versions.get(&block.into()) else {
        return (None, 0);
    };
    let now = written
        .last()
        .and_then(|v| kept.work.contents.get(&(block.into(), v.at)).cloned());
    (now, written.len())
}

#[test]
fn what_the_outside_says_is_written_into_each_work_whose_block_does_not_say_it() {
    let mut h = H::with(vec![watched(), short()]);
    let (_, a) = h.add(new_item("a", &[], "watched"));
    let (_, b) = h.add(new_item("b", &[], "watched"));
    // A work whose flow has no block of the source is written nothing.
    let (_, other) = h.add(new_item("c", &[], "short"));
    let before = h.d().work(other).unwrap().work.records.clone();
    assert_eq!(h.outside("main", "Main is at 1."), vec![], "the works are paused");
    for work in [a, b] {
        assert_eq!(said(&h, work, "main"), (Some("Main is at 1.".to_string()), 1));
    }
    assert_eq!(h.d().work(other).unwrap().work.records, before);
    // Said again, it is written nowhere; said anew, it is written again.
    h.outside("main", "Main is at 1.");
    assert_eq!(said(&h, a, "main").1, 1);
    h.outside("main", "Main is at 2.");
    assert_eq!(said(&h, a, "main"), (Some("Main is at 2.".to_string()), 2));
}

#[test]
fn a_work_let_on_runs_the_cell_that_reads_the_block_and_one_that_ended_is_written_nothing() {
    let mut h = H::with(vec![watched()]);
    let (_, work) = h.add(new_item("a", &[], "watched"));
    h.outside("main", "Main is at 1.");
    let todos = h.let_on(work);
    assert_eq!(rounds(&todos), [(work, "look".to_string(), 1)]);
    h.through(work);
    assert_eq!(h.decision(work), Decision::Ended(End::Merged));
    assert_eq!(h.outside("main", "Main is at 2."), vec![]);
    assert_eq!(said(&h, work, "main"), (Some("Main is at 1.".to_string()), 1));
}

#[test]
fn the_drivers_own_source_is_not_written_from_outside() {
    let mut h = H::new();
    let (_, work) = h.add(new_item("a", &[], "short"));
    let before = h.dependencies(work);
    assert_eq!(h.outside(driver::DEPENDENCIES, "merged"), vec![]);
    assert_eq!(h.dependencies(work), before);
}
