//! What the check before the merge writes of the paths the installation names for the owner's approval: the files the
//! commit it checked changes under them, with their content, written only when that list changes, so that the owner's
//! yes holds until a commit changes those files again, and a work that changes none of them writes nothing.

use super::*;
use crate::git::tests::{Repo, commit, run};
use crate::host::Local;

const WORK: WorkNumber = WorkNumber(1);

/// A check of this commit against main at `beef`.
fn checked(commit: &str) -> Checked {
    Checked {
        commit: commit.to_string(),
        main: "beef".to_string(),
        failed: None,
        by: None,
        took_in: false,
        notes: Vec::new(),
    }
}

fn block() -> BlockId {
    "owner_paths".into()
}

/// What the check writes of these files, after the block said `before`.
fn written(before: Option<&str>, files: &[&str], commit: &str) -> Option<String> {
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    owned_in(&block(), before, &files, &checked(commit), "main").map(|h| {
        assert_eq!((h.block, h.matters, h.value), (block(), true, None));
        h.content
    })
}

#[test]
fn a_work_that_changes_none_of_the_owners_paths_writes_nothing_there() {
    assert_eq!(written(None, &[], "c0ffee"), None);
    assert_eq!(written(Some(""), &[], "c0ffee"), None);
}

#[test]
fn the_files_changed_under_the_owners_paths_are_written_with_the_commit() {
    let files = ["flow/a.toml at 1111", "flow/b.toml deleted"];
    assert_eq!(
        written(None, &files, "c0ffee").as_deref(),
        Some(
            "The commit c0ffee changes, against main at beef, these files under the paths the owner approves:\n- \
             flow/a.toml at 1111\n- flow/b.toml deleted"
        )
    );
}

#[test]
fn a_later_commit_that_leaves_those_files_as_they_were_writes_nothing_and_one_that_changes_them_writes_again() {
    let files = ["flow/a.toml at 1111"];
    let before = written(None, &files, "c0ffee").unwrap();
    assert_eq!(written(Some(&before), &files, "decade"), None);
    let again = written(Some(&before), &["flow/a.toml at 2222"], "decade").unwrap();
    assert!(
        again.contains("decade") && again.ends_with("- flow/a.toml at 2222"),
        "{again}"
    );
}

#[test]
fn a_work_that_takes_back_its_change_to_those_files_writes_the_block_blank() {
    let before = written(None, &["flow/a.toml at 1111"], "c0ffee").unwrap();
    assert_eq!(written(Some(&before), &[], "decade").as_deref(), Some(""));
}

#[tokio::test]
async fn the_files_are_those_the_commit_changes_under_the_paths_with_the_blob_it_holds() {
    let mut repo = Repo::new("true");
    repo.project.owner_approves = vec!["flow/".to_string(), "docs/*.md".to_string()];
    let main = git::git(&repo.project.repo, &["rev-parse", "main"]).await.unwrap();
    let place = repo.place(WORK);
    let host = Local::new(repo.project.clone());
    start(&repo.project, &host, &place, &Mutex::new(())).await.unwrap();
    for dir in ["flow", "docs/deep", "src"] {
        std::fs::create_dir_all(place.dir.join(dir)).unwrap();
    }
    commit(&place.dir, "flow/a.toml", "a = 1\n");
    commit(&place.dir, "docs/spec.md", "Spec.\n");
    commit(&place.dir, "docs/deep/other.md", "Not named.\n");
    let head = commit(&place.dir, "src/main.rs", "fn main() {}\n");
    let checked = Checked { main, ..checked(&head) };
    let files = owned_files(&repo.project, &checked).await.unwrap();
    let blob = |file: &str| run(&place.dir, &["rev-parse", &format!("{head}:{file}")]);
    let (spec, flow) = (blob("docs/spec.md"), blob("flow/a.toml"));
    assert_eq!(
        files,
        [format!("docs/spec.md at {spec}"), format!("flow/a.toml at {flow}")]
    );
    repo.project.owner_approves.clear();
    assert_eq!(
        owned_files(&repo.project, &checked).await.unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn only_the_check_names_a_block_of_the_owners_paths_and_it_is_no_choice() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
    let has = flow::Has {
        sources: Default::default(),
        acts: ["start", "check", "merge"].map(String::from).into(),
    };
    let flow = flow::compile(&flow::read(&dir).unwrap(), &has).unwrap();
    let named = |cell: &str, block: &str| {
        let mut flow = flow.clone();
        let cell = flow.cells.iter_mut().find(|c| c.id.as_str() == cell).unwrap();
        cell.owner_paths = Some(block.into());
        faults(&flow)
    };
    assert_eq!(named("check", "failures"), Vec::<String>::new());
    let merging = named("merge", "merged");
    assert!(
        merging.len() == 1 && merging[0].contains("only the check"),
        "{merging:?}"
    );
    let choosing = named("check", "passed");
    assert!(choosing.iter().any(|f| f.contains("it is a choice")), "{choosing:?}");
}
