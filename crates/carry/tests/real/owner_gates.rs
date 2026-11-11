//! The owner's gate at the merge on the paths the installation names for the owner's yes: the check writes the files
//! the work changes under them into the block its cell names, and the merge waits until the owner confirmed it; a work
//! that changes none of them is never held.

use decide::{Decision, Waited};
use service::Act;
use store::WorkNumber;

use crate::{Fake, On, Real, compile, ended, git_in, source};

/// The paths the installation names for the owner's yes.
const OWNERS: &str = "flow/";

/// The flow here, with the check writing the files under the owner's paths into `owned`, and the merge waiting, besides
/// the check's pass, until the owner confirmed what `owned` lists, in `owner_approved`.
fn gated() -> flow::Compiled {
    let mut source = source(On::Acts);
    source.flow.push_str(
        "\n[blocks.owned]\nshown = true\n\n[blocks.owner_approved]\nvalues = [\"yes\"]\npeople = true\n\n\
         [tests.owner_approved]\nsays = \"The owner approved what the work changes under the owner's paths\"\n\
         kind = \"confirmed\"\nreads = [\"owned\"]\nby = \"owner_approved\"\nis = \"yes\"\n",
    );
    let edit = |text: &mut String, from: &str, to: &str| {
        assert!(text.contains(from), "{from} in {text}");
        *text = text.replace(from, to);
    };
    let check = source.cells.get_mut("check").expect("the flow has a check");
    edit(
        check,
        r#"outputs = ["passed", "failures"]"#,
        "outputs = [\"passed\", \"failures\", \"owned\"]\nowner_paths = \"owned\"",
    );
    let merge = source.cells.get_mut("merge").expect("the flow has a merge");
    edit(
        merge,
        r#"requires = ["passed"]"#,
        r#"requires = ["passed", "owner_approved"]"#,
    );
    compile(&source)
}

/// The loop on the gated flow, its build writing this file.
fn opened(writes: &'static str) -> Real {
    let fake = Fake {
        build_writes: Some(writes),
        ..Fake::default()
    };
    Real::open_with(gated(), fake, None, "true", vec![OWNERS.to_string()])
}

/// Whether the work waits at the merge for the owner's approval.
fn waits_for_the_owner(real: &Real, work: WorkNumber) -> bool {
    match real.decision(work) {
        Decision::Wait { at, on } => {
            at.as_str() == "merge"
                && (on.iter())
                    .any(|w| matches!(w, Waited::Precondition { test, .. } if test.as_str() == "owner_approved"))
        }
        _ => false,
    }
}

/// The content of the block's current version, where it has one.
fn current(real: &Real, work: WorkNumber, block: &str) -> Option<String> {
    let kept = real.work(work);
    let at = decide::versions(&kept.records).get(&block.into())?.last()?.at;
    kept.contents.get(&(block.into(), at)).cloned()
}

#[tokio::test]
async fn a_change_under_the_owners_paths_merges_only_once_the_owner_approved_the_files_the_check_listed() {
    let mut real = opened("flow/greeting.toml");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| waits_for_the_owner(real, work)).await;
    assert_eq!(real.finished(work), ["start", "build", "check"]);
    let place = real.place(work);
    let head = git_in(&real.repo, &["rev-parse", &place.reference()]);
    let blob = git_in(&real.repo, &["rev-parse", &format!("{head}:flow/greeting.toml")]);
    let owned = current(&real, work, "owned").expect("the check wrote the files");
    assert!(
        owned.starts_with(&format!("The commit {head} changes, against main at "))
            && owned.ends_with(&format!(":\n- flow/greeting.toml at {blob}")),
        "{owned}"
    );
    let log = git_in(&real.repo, &["log", "main", "--format=%s"]);
    assert!(!log.contains("Build the greeting"), "nothing merged: {log}");

    real.act(
        work,
        Act::Write {
            block: "owner_approved".into(),
            content: String::new(),
            value: Some("yes".to_string()),
        },
    );
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    let log = git_in(&real.repo, &["log", "main", "--format=%s"]);
    assert!(log.lines().any(|line| line == "Build the greeting"), "{log}");
}

#[tokio::test]
async fn a_work_that_changes_nothing_under_the_owners_paths_writes_nothing_there_and_is_not_held() {
    let mut real = opened("src/greeting.txt");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    assert_eq!(current(&real, work, "owned"), None);
}
