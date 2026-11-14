//! The paths the project protects (`project.toml`, `[protected]`), as a work's change touches them (principles
//! `protected-files-are-the-owners` and `named-paths-wait-for-the-owner`). As a round of a cell whose line may change
//! the work ends, the engine looks at what the work's branch changes against where it left main under those paths, and
//! asks the owner, by itself, whether the change stays: the question of `agents/asks.toml` (`[protected]`), showing the
//! files and the change, which waits, so that the work stops at the cell until the owner answers, and the merge waits
//! while it stands unanswered. It is asked once for each change: asked again under its name only where the change is
//! another, and withdrawn once the work touches no such path. The question is the engine's, beside the agent's return,
//! and no agent writes it.

use decide::questions::questions;
use flow::agents::{AskProtected, fill};
use work::{Question, Way, Work};

use crate::git::{self, Place};

/// What the engine asks of the work's change under the protected paths, where it is to ask anything: the question
/// asked, or asked anew for another change, or withdrawn where the work touches none of the paths; none where the
/// question as it stands already says what it would, or the project protects no path. `main` is the reference that
/// holds main's head; the change is the branch's against where it left main.
pub(crate) async fn look(
    place: &Place,
    main: &str,
    paths: &[String],
    words: &AskProtected,
    work: &Work,
) -> Result<Option<Question>, String> {
    if paths.is_empty() {
        return Ok(None);
    }
    let branch = place.reference();
    let base = git::git(&place.dir, &["merge-base", main, &branch]).await?;
    let mut args = vec!["diff", "--no-renames", "--name-status", &base, &branch, "--"];
    args.extend(paths.iter().map(String::as_str));
    let named = git::git(&place.dir, &args).await?;
    let standing = (questions(&work.records).get(words.name.as_str()))
        .filter(|q| q.latest.engine)
        .map(|q| (q.stands(), work.question(q.latest)));
    if named.trim().is_empty() {
        return Ok(match standing {
            Some((true, _)) => Some(Question::Withdrawn {
                name: words.name.clone(),
                why: words.withdrawn.clone(),
            }),
            _ => None,
        });
    }
    let files: Vec<String> = (named.lines())
        .filter_map(|line| {
            let (status, path) = line.split_once('\t')?;
            let change = match status.chars().next() {
                Some('A') => &words.added,
                Some('D') => &words.deleted,
                _ => &words.changed,
            };
            Some(fill(&words.file, &[("path", path), ("change", change)]))
        })
        .collect();
    args.retain(|arg| *arg != "--name-status");
    let diff = git::git(&place.dir, &args).await?;
    let ask = fill(&words.ask, &[("files", &files.join("\n")), ("diff", &diff)]);
    if let Some((true, Some(Question::Asked { ask: asked, .. }))) = standing
        && *asked == ask
    {
        return Ok(None);
    }
    Ok(Some(Question::Asked {
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
    }))
}

#[cfg(test)]
mod tests {
    use decide::{Questioned, Returned, RunId, Seq, Started};
    use store::WorkNumber;
    use work::Back;

    use super::*;
    use crate::git::tests::{Repo, commit, run};

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

    /// Writes the file in the work's worktree, its directory made where it has none, and commits it.
    fn commits(place: &Place, file: &str, text: &str) {
        if let Some(dir) = place.dir.join(file).parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        commit(&place.dir, file, text);
    }

    /// A work whose one run's round ended with the engine asking the question, kept as the loop keeps it.
    fn asked_with(question: Question) -> Work {
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
        let (_repo, place) = started().await;
        commits(&place, "greeting.txt", "Hello, you.\n");
        commits(&place, "flows/not.toml", "x = 1\n");
        let asked = look(&place, "main", &paths(), &words(), &Work::default()).await;
        assert_eq!(asked, Ok(None));
    }

    #[tokio::test]
    async fn a_change_that_touches_a_protected_path_is_asked_of_the_owner_with_the_files_and_the_change() {
        let (_repo, place) = started().await;
        commits(&place, "flow/cell.toml", "title = \"A cell\"\n");
        commits(&place, "greeting.txt", "Hello, you.\n");
        let Ok(Some(Question::Asked {
            name,
            waits,
            recommends,
            ask,
            options,
        })) = look(&place, "main", &paths(), &words(), &Work::default()).await
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
    }

    #[tokio::test]
    async fn the_same_change_is_asked_once_and_another_change_asks_again() {
        let (_repo, place) = started().await;
        commits(&place, "AGENTS.md", "Rules.\n");
        let first = look(&place, "main", &paths(), &words(), &Work::default())
            .await
            .unwrap()
            .unwrap();
        let work = asked_with(first.clone());
        assert_eq!(look(&place, "main", &paths(), &words(), &work).await, Ok(None));
        // A commit elsewhere leaves the change as it was.
        commits(&place, "greeting.txt", "Hello, you.\n");
        assert_eq!(look(&place, "main", &paths(), &words(), &work).await, Ok(None));
        commits(&place, "AGENTS.md", "Other rules.\n");
        let again = look(&place, "main", &paths(), &words(), &work).await.unwrap();
        let Some(Question::Asked { ask, .. }) = again else {
            panic!("another change is not asked: {again:?}");
        };
        assert!(ask.contains("+Other rules."), "{ask}");
        assert_ne!(
            Some(&ask),
            match &first {
                Question::Asked { ask, .. } => Some(ask),
                Question::Withdrawn { .. } => None,
            }
        );
    }

    #[tokio::test]
    async fn a_change_taken_back_withdraws_the_question_that_stands_and_asks_nothing_once_withdrawn() {
        let (_repo, place) = started().await;
        commits(&place, "flow/cell.toml", "title = \"A cell\"\n");
        let asked = look(&place, "main", &paths(), &words(), &Work::default())
            .await
            .unwrap()
            .unwrap();
        run(&place.dir, &["rm", "-q", "flow/cell.toml"]);
        run(&place.dir, &["commit", "-q", "-m", "Take the cell back"]);
        let work = asked_with(asked);
        let withdrawn = look(&place, "main", &paths(), &words(), &work).await;
        assert_eq!(
            withdrawn,
            Ok(Some(Question::Withdrawn {
                name: "protected_files".to_string(),
                why: words().withdrawn,
            }))
        );
        let work = asked_with(withdrawn.unwrap().unwrap());
        assert_eq!(look(&place, "main", &paths(), &words(), &work).await, Ok(None));
        assert_eq!(
            look(&place, "main", &paths(), &words(), &Work::default()).await,
            Ok(None)
        );
    }

    #[tokio::test]
    async fn a_project_that_protects_no_path_asks_nothing() {
        let (_repo, place) = started().await;
        commits(&place, "AGENTS.md", "Rules.\n");
        assert_eq!(look(&place, "main", &[], &words(), &Work::default()).await, Ok(None));
    }
}
