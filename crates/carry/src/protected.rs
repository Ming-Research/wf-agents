//! The paths the project protects (`project.toml`, `[protected]`), as a work's change touches them (principles
//! `protected-files-are-the-owners` and `named-paths-wait-for-the-owner`): a change to them waits at the merge for the
//! owner's yes.
//!
//! The work's own change under those paths is what its branch's head holds there that main, where the branch left it,
//! does not: each file it adds, changes or deletes, by its path and the content it ends with, so that main moving on
//! asks nothing again where the work's own change is the same. The engine asks the owner of it by itself, in the
//! question of `agents/asks.toml` (`[protected]`), which shows the files and the change, waits, and recommends none of
//! its options: asked again under its name where the change is another, withdrawn once the work touches none of the
//! paths. The question is the engine's, beside what the round returns, and no agent writes it.
//!
//! The guarantee is at the merge: the check before it looks at the change of the exact head it checks, and does not
//! pass while that change is not empty and is not one the owner let merge, by the option `merges` names; it asks, or
//! asks again, where the question does not stand unanswered for that change. A look the check cannot make fails the
//! check. As a round of a line that may change the work ends, the engine looks as well, so that the owner is asked
//! early; a look that fails there asks nothing, the check being what holds the merge.
//!
//! Git is run with no program the repository's configuration or attributes could name, and so that what attributes
//! say of a file cannot keep its content out of the change shown: the configuration is refused first as the acts
//! refuse it, and the diff is given as text, with no external diff and no conversion.

use decide::questions::questions;
use flow::agents::{AskProtected, fill};
use work::{Question, Way, Work};

use crate::git::{self, Place};
use crate::settings::Project;

/// The options every diff of the look is given: no program the configuration names, no conversion an attribute names,
/// and every file shown as text.
const SAFE_DIFF: [&str; 4] = ["--no-ext-diff", "--no-textconv", "--text", "--no-renames"];

/// The work's own change under the protected paths, between where it left main and its head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    /// Each file, by its path, with its kind of change: `A`, `M` or `D` as git says it, in the order git gives them.
    pub(crate) files: Vec<(char, String)>,
    /// The change told apart from any other: each file's path and the content it ends with, one to a line, in order.
    pub(crate) key: String,
    /// The change itself, as text.
    pub(crate) diff: String,
}

/// The change under the paths between `base` and `head`, as the work's worktree holds the repository; empty where the
/// project protects no path. The configuration that could make git run a program is refused first.
pub(crate) async fn change(
    project: &Project,
    place: &Place,
    base: &str,
    head: &str,
    paths: &[String],
) -> Result<Change, String> {
    let empty = Change {
        files: Vec::new(),
        key: String::new(),
        diff: String::new(),
    };
    if paths.is_empty() {
        return Ok(empty);
    }
    git::configuration(project, place).await?;
    let mut args: Vec<&str> = vec!["diff"];
    args.extend(SAFE_DIFF);
    args.extend(["--raw", "--no-abbrev", base, head, "--"]);
    args.extend(paths.iter().map(String::as_str));
    let raw = git::git(&place.dir, &args).await?;
    let mut files = Vec::new();
    let mut key = Vec::new();
    for line in raw.lines() {
        // `:<mode> <mode> <blob> <blob> <status>`, a tab, and the path.
        let Some((fields, path)) = line.split_once('\t') else {
            continue;
        };
        let fields: Vec<&str> = fields.split_whitespace().collect();
        let (Some(blob), Some(status)) = (fields.get(3), fields.get(4).and_then(|s| s.chars().next())) else {
            continue;
        };
        files.push((status, path.to_string()));
        key.push(format!("{path} {blob}"));
    }
    if files.is_empty() {
        return Ok(empty);
    }
    args.retain(|arg| !matches!(*arg, "--raw" | "--no-abbrev"));
    let diff = git::git(&place.dir, &args).await?;
    Ok(Change {
        files,
        key: key.join("\n"),
        diff,
    })
}

/// The work's own change under the protected paths at this commit, against where it left main as `main` holds it.
pub(crate) async fn change_at(
    project: &Project,
    place: &Place,
    main: &str,
    head: &str,
    paths: &[String],
) -> Result<Change, String> {
    if paths.is_empty() {
        return change(project, place, "", "", paths).await;
    }
    let base = git::git(&place.dir, &["merge-base", main, head]).await?;
    change(project, place, &base, head, paths).await
}

/// The engine's question of the change as it stands in the work: whether it stands, the change its latest version is
/// of, and the option that version was answered with.
struct Standing {
    stands: bool,
    about: Option<String>,
    answer: Option<u64>,
}

fn standing(work: &Work, words: &AskProtected) -> Option<Standing> {
    let all = questions(&work.records);
    let q = all.get(words.name.as_str()).filter(|q| q.latest.engine)?;
    let about = match work.question(q.latest) {
        Some(Question::Asked { about, .. }) => about.clone(),
        _ => None,
    };
    Some(Standing {
        stands: q.stands(),
        about,
        answer: q.answer.map(|a| a.option),
    })
}

/// Whether the owner let this change merge: the engine's question stands of this very change, and its latest version
/// was answered with the option `merges` names.
pub(crate) fn let_merge(work: &Work, words: &AskProtected, change: &Change) -> bool {
    standing(work, words)
        .is_some_and(|s| s.stands && s.about.as_deref() == Some(change.key.as_str()) && s.answer == Some(words.merges))
}

/// What the engine asks of the change, where it is to ask anything: the question asked, or asked anew where the one
/// that stands is of another change or was answered with another option than the one that lets it merge, or withdrawn
/// where the work's change is empty and one stands; none where the question stands unanswered of this change, or was
/// answered letting it merge.
pub(crate) fn asking(work: &Work, words: &AskProtected, change: &Change) -> Option<Question> {
    let standing = standing(work, words);
    if change.files.is_empty() {
        return standing.filter(|s| s.stands).map(|_| Question::Withdrawn {
            name: words.name.clone(),
            why: words.withdrawn.clone(),
        });
    }
    let same = (standing.as_ref()).is_some_and(|s| s.stands && s.about.as_deref() == Some(change.key.as_str()));
    let stays = same && (standing.as_ref()).is_some_and(|s| s.answer.is_none() || s.answer == Some(words.merges));
    if stays {
        return None;
    }
    let files: Vec<String> = (change.files.iter())
        .map(|(status, path)| {
            let kind = match status {
                'A' => &words.added,
                'D' => &words.deleted,
                _ => &words.changed,
            };
            fill(&words.file, &[("path", path), ("change", kind)])
        })
        .collect();
    let ask = fill(&words.ask, &[("files", &files.join("\n")), ("diff", &change.diff)]);
    Some(Question::Asked {
        name: words.name.clone(),
        waits: true,
        recommends: None,
        ask,
        options: (words.options.iter())
            .map(|o| Way {
                does: o.does.clone(),
                follows: o.follows.clone(),
            })
            .collect(),
        about: Some(change.key.clone()),
    })
}

/// What the engine asks of the work's change under the protected paths at the head of its branch as a round ends,
/// where it asks anything: the early notice of what the check before the merge holds the work to.
pub(crate) async fn look(
    project: &Project,
    place: &Place,
    main: &str,
    paths: &[String],
    words: &AskProtected,
    work: &Work,
) -> Result<Option<Question>, String> {
    let change = change_at(project, place, main, &place.reference(), paths).await?;
    Ok(asking(work, words, &change))
}

#[cfg(test)]
pub(crate) mod tests {
    use decide::{Answered, Questioned, Returned, RunId, Seq, Started};
    use store::WorkNumber;
    use work::Back;

    use super::*;
    use crate::git::tests::{Repo, commit, marking, run};

    const WORK: WorkNumber = WorkNumber(1);

    /// The paths the tests' project protects.
    fn paths() -> Vec<String> {
        vec!["AGENTS.md".to_string(), "flow/".to_string()]
    }

    /// The question's words, as the flow crate's tests write them.
    fn words() -> AskProtected {
        crate::returns::tests::words().asks.protected
    }

    /// A started work, its place.
    async fn started() -> (Repo, Place) {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        (repo, started.place)
    }

    /// Writes the file in the directory, its own directory made where it has none, and commits it.
    fn commits(dir: &std::path::Path, file: &str, text: &str) {
        if let Some(parent) = dir.join(file).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        commit(dir, file, text);
    }

    /// What the engine asks as a round ends with the work's branch as it is.
    async fn looked(repo: &Repo, place: &Place, work: &Work) -> Result<Option<Question>, String> {
        look(&repo.project, place, "main", &paths(), &words(), work).await
    }

    /// A work whose one run's round ended with the engine asking the question, kept as the loop keeps it, answered
    /// with this option where one is given.
    pub(crate) fn asked_with(question: Question, answer: Option<u64>) -> Work {
        let run = RunId(1);
        let mut work = Work::default();
        work.records.started.push(Started {
            run,
            cell: "build".into(),
            at: Seq(1),
            because: Vec::new(),
            basis: Default::default(),
        });
        work.records.returned.push(Returned { run, at: Seq(2) });
        let withdrawn = matches!(question, Question::Withdrawn { .. });
        work.records.questions.push(Questioned {
            name: question.name().to_string(),
            cell: "build".into(),
            run,
            engine: true,
            waits: !withdrawn,
            withdrawn,
            at: Seq(3),
        });
        if let Some(option) = answer {
            work.records.answers.push(Answered {
                question: question.name().to_string(),
                version: Seq(3),
                option,
                why: None,
                at: Seq(4),
            });
        }
        let back = Back::Returned {
            dealt_with: Vec::new(),
            handed: Vec::new(),
            asks: Vec::new(),
            found: Vec::new(),
            said: Default::default(),
            engine: vec![question],
        };
        work.back.insert((run, 1), back);
        work
    }

    #[tokio::test]
    async fn a_change_that_touches_no_protected_path_asks_nothing() {
        let (repo, place) = started().await;
        commits(&place.dir, "greeting.txt", "Hello, you.\n");
        commits(&place.dir, "flows/not.toml", "x = 1\n");
        assert_eq!(looked(&repo, &place, &Work::default()).await, Ok(None));
    }

    #[tokio::test]
    async fn a_change_that_touches_a_protected_path_is_asked_of_the_owner_with_the_files_and_the_change() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        commits(&place.dir, "greeting.txt", "Hello, you.\n");
        let Ok(Some(Question::Asked {
            name,
            waits,
            recommends,
            ask,
            options,
            about,
        })) = looked(&repo, &place, &Work::default()).await
        else {
            panic!("nothing is asked");
        };
        assert_eq!(name, "protected_files");
        assert!(waits, "the work waits for the answer");
        assert_eq!(recommends, None, "the engine recommends nothing");
        assert!(ask.starts_with("May this merge?\n- flow/cell.toml: added\n"), "{ask}");
        assert!(ask.contains("+title = \"A cell\""), "the change is shown: {ask}");
        assert!(!ask.contains("greeting.txt"), "only what is protected is shown: {ask}");
        assert_eq!(options.len(), 2);
        let about = about.expect("the question says what change it is of");
        assert!(about.starts_with("flow/cell.toml "), "{about}");
    }

    #[tokio::test]
    async fn the_same_change_is_asked_once_and_another_change_asks_again() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let first = looked(&repo, &place, &Work::default()).await.unwrap().unwrap();
        let work = asked_with(first.clone(), None);
        assert_eq!(looked(&repo, &place, &work).await, Ok(None));
        // A commit elsewhere leaves the change as it was.
        commits(&place.dir, "greeting.txt", "Hello, you.\n");
        assert_eq!(looked(&repo, &place, &work).await, Ok(None));
        commits(&place.dir, "AGENTS.md", "Other rules.\n");
        let again = looked(&repo, &place, &work).await.unwrap();
        let Some(Question::Asked { ask, about, .. }) = again else {
            panic!("another change is not asked: {again:?}");
        };
        assert!(ask.contains("+Other rules."), "{ask}");
        let Question::Asked { about: before, .. } = &first else {
            panic!("the first is asked");
        };
        assert_ne!(&about, before);
    }

    #[tokio::test]
    async fn main_moving_on_asks_nothing_again_where_the_works_own_change_is_the_same() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        let first = looked(&repo, &place, &Work::default()).await.unwrap().unwrap();
        let work = asked_with(first, None);
        // Main changes a protected file the work does not touch, and another file.
        commits(&repo.project.repo, "AGENTS.md", "Main's rules.\n");
        commits(&repo.project.repo, "greeting.txt", "Hello from main.\n");
        assert_eq!(looked(&repo, &place, &work).await, Ok(None), "before main is taken in");
        run(&place.dir, &["merge", "-q", "--no-edit", "main"]);
        assert_eq!(looked(&repo, &place, &work).await, Ok(None), "after main is taken in");
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        let files: Vec<&str> = change.files.iter().map(|(_, path)| path.as_str()).collect();
        assert_eq!(files, ["flow/cell.toml"], "main's own change is not the work's");
    }

    #[tokio::test]
    async fn a_change_taken_back_withdraws_the_question_that_stands_and_asks_nothing_once_withdrawn() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        let asked = looked(&repo, &place, &Work::default()).await.unwrap().unwrap();
        run(&place.dir, &["rm", "-q", "flow/cell.toml"]);
        run(&place.dir, &["commit", "-q", "-m", "Take the cell back"]);
        let work = asked_with(asked, None);
        let withdrawn = looked(&repo, &place, &work).await;
        assert_eq!(
            withdrawn,
            Ok(Some(Question::Withdrawn {
                name: "protected_files".to_string(),
                why: words().withdrawn,
            }))
        );
        let work = asked_with(withdrawn.unwrap().unwrap(), None);
        assert_eq!(looked(&repo, &place, &work).await, Ok(None));
        assert_eq!(looked(&repo, &place, &Work::default()).await, Ok(None));
    }

    #[tokio::test]
    async fn only_the_option_that_lets_it_merge_lets_this_very_change_merge_and_any_other_asks_anew() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let asked = looked(&repo, &place, &Work::default()).await.unwrap().unwrap();
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        let words = words();
        let unanswered = asked_with(asked.clone(), None);
        assert!(!let_merge(&unanswered, &words, &change), "not answered");
        assert_eq!(asking(&unanswered, &words, &change), None, "it stands unanswered");
        let let_it = asked_with(asked.clone(), Some(words.merges));
        assert!(let_merge(&let_it, &words, &change));
        assert_eq!(asking(&let_it, &words, &change), None);
        let other = if words.merges == 1 { 2 } else { 1 };
        let taken_back = asked_with(asked, Some(other));
        assert!(!let_merge(&taken_back, &words, &change), "another option is no yes");
        let anew = asking(&taken_back, &words, &change);
        assert!(
            matches!(&anew, Some(Question::Asked { about, .. }) if about.as_deref() == Some(change.key.as_str())),
            "the change still there is asked anew: {anew:?}"
        );
        // A yes to this change lets no other merge.
        commits(&place.dir, "AGENTS.md", "Other rules.\n");
        let other = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        assert!(!let_merge(&let_it, &words, &other));
    }

    #[tokio::test]
    async fn a_project_that_protects_no_path_asks_nothing() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let asked = look(&repo.project, &place, "main", &[], &words(), &Work::default()).await;
        assert_eq!(asked, Ok(None));
    }

    #[tokio::test]
    async fn a_configuration_that_names_a_program_is_refused_before_the_look() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let marked = repo.data.join("external-ran");
        let program = repo.data.join("external");
        marking(&program, &marked);
        run(
            &repo.project.repo,
            &["config", "diff.external", &program.display().to_string()],
        );
        let refused = looked(&repo, &place, &Work::default()).await;
        assert!(
            matches!(&refused, Err(why) if why.contains("diff.external")),
            "{refused:?}"
        );
        assert!(!marked.exists(), "the external diff was run");
    }

    #[tokio::test]
    async fn an_external_diff_allowed_by_the_project_is_still_not_run() {
        let (mut repo, place) = started().await;
        repo.project.allowed_config = vec!["diff.external".to_string()];
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let marked = repo.data.join("external-ran");
        let program = repo.data.join("external");
        marking(&program, &marked);
        run(
            &repo.project.repo,
            &["config", "diff.external", &program.display().to_string()],
        );
        let asked = looked(&repo, &place, &Work::default()).await;
        assert!(
            matches!(&asked, Ok(Some(Question::Asked { ask, .. })) if ask.contains("+Rules.")),
            "{asked:?}"
        );
        assert!(!marked.exists(), "the external diff was run");
    }

    #[tokio::test]
    async fn attributes_that_hide_a_files_diff_do_not_hide_it_from_the_owner() {
        let (repo, place) = started().await;
        commits(&place.dir, ".gitattributes", "flow/** -diff\nAGENTS.md -diff\n");
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        let asked = looked(&repo, &place, &Work::default()).await;
        let Ok(Some(Question::Asked { ask, .. })) = asked else {
            panic!("nothing is asked: {asked:?}");
        };
        assert!(ask.contains("+title = \"A cell\""), "the content is shown: {ask}");
        assert!(!ask.contains("Binary files"), "{ask}");
    }
}
