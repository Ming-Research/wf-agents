//! The paths the project protects (`project.toml`, `[protected]`), as a work's change touches them (principles
//! `protected-files-are-the-owners` and `named-paths-wait-for-the-owner`): the work says why each such change is
//! needed, or takes it back, and the change waits at the merge for the owner's yes.
//!
//! The work's own change under those paths is what its branch's head holds there that main, where the branch left it,
//! does not: each file it adds, changes or deletes, by its path and the content it ends with, so that main moving on
//! asks nothing again where the work's own change is the same. The engine asks the owner of it by itself, in the
//! question of `agents/asks.toml` (`[protected]`), which shows each file with why the work says it is needed and its
//! change, waits, and recommends none of its options: asked again under its name where the change is another, or where
//! the work gives another reason for it before the owner answered, withdrawn once the work touches none of the paths.
//! The owner's yes to the change holds while the change is the same, whatever reason is given for it after. The question is the engine's,
//! beside what the round returns, and no agent writes it.
//!
//! Why a file is changed is the work's, said in the field `protected_changes` of a return that does the cell's work,
//! under the file's path, and it stands for the file as it then is: the question asked carries it, and a later version
//! asks with it again while the file is as it was. As a round of a line that may change the work ends, a file of the
//! change that neither its return nor the question that stands says why of, and that the owner did not let merge as it
//! is, is unexplained: its return is sent back for it, where it does the cell's work, and nothing is asked until no file
//! is, so that the owner is asked once, with the reasons.
//!
//! The guarantee is at the merge: the check before it looks at the change of the exact head it checks, and does not
//! pass while that change is not empty and is not one the owner let merge, by the option `merges` names; it asks, or
//! asks again, where the question does not stand unanswered for that change, with the reasons the question that stands
//! says and a file no round explained said to have none. A look the check cannot make fails the check. As a round of
//! a line that may change the work ends, the engine looks as well, so that the owner is asked early; a look that fails
//! there asks nothing and sends nothing back, the check being what holds the merge.
//!
//! Git is run with no program the repository's configuration or attributes could name, and so that what attributes
//! say of a file cannot keep its content out of the change shown: the configuration is refused first as the acts
//! refuse it, and the diff is given as text, with no external diff and no conversion. Each path git gives is taken as
//! it is, never as a pattern.

use std::collections::BTreeMap;

use decide::questions::questions;
use flow::agents::{AskProtected, fill};
use work::{Question, Way, Work};

use crate::git::{self, Place};
use crate::settings::Project;

/// The options every diff of the look is given: no program the configuration names, no conversion an attribute names,
/// and every file shown as text.
const SAFE_DIFF: [&str; 4] = ["--no-ext-diff", "--no-textconv", "--text", "--no-renames"];

/// A file of the work's own change under the protected paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct File {
    /// Its kind of change: `A`, `M` or `D` as git says it.
    pub(crate) status: char,
    pub(crate) path: String,
    /// The content it ends with, as git names it.
    pub(crate) blob: String,
    /// Its change, as text.
    pub(crate) diff: String,
}

impl File {
    /// The file's change told apart from any other: its path and the content it ends with.
    fn line(&self) -> String {
        format!("{} {}", self.path, self.blob)
    }
}

/// The work's own change under the protected paths, between where it left main and its head.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Change {
    /// Each file it touches, in the order git gives them.
    pub(crate) files: Vec<File>,
}

impl Change {
    /// The change told apart from any other: each file's path and the content it ends with, one to a line, in order.
    pub(crate) fn key(&self) -> String {
        let lines: Vec<String> = self.files.iter().map(File::line).collect();
        lines.join("\n")
    }
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
    if paths.is_empty() {
        return Ok(Change::default());
    }
    git::configuration(project, place).await?;
    let mut args: Vec<&str> = vec!["diff"];
    args.extend(SAFE_DIFF);
    args.extend(["--raw", "-z", "--no-abbrev", base, head, "--"]);
    args.extend(paths.iter().map(String::as_str));
    let raw = git::git(&place.dir, &args).await?;
    // `:<mode> <mode> <blob> <blob> <status>` and the path, each ended by a NUL.
    let mut parts = raw.split('\0');
    let mut files = Vec::new();
    while let (Some(fields), Some(path)) = (parts.next(), parts.next()) {
        let fields: Vec<&str> = fields.split_whitespace().collect();
        let (Some(blob), Some(status)) = (fields.get(3), fields.get(4).and_then(|s| s.chars().next())) else {
            continue;
        };
        let literal = format!(":(literal){path}");
        let mut args: Vec<&str> = vec!["diff"];
        args.extend(SAFE_DIFF);
        args.extend([base, head, "--", literal.as_str()]);
        let diff = git::git(&place.dir, &args).await?;
        files.push(File {
            status,
            path: path.to_string(),
            blob: blob.to_string(),
            diff,
        });
    }
    Ok(Change { files })
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
        return Ok(Change::default());
    }
    let base = git::git(&place.dir, &["merge-base", main, head]).await?;
    change(project, place, &base, head, paths).await
}

/// The engine's question of the change as it stands in the work: whether it stands, the change its latest version is
/// of, why that version says each file is changed, and the option that version was answered with.
struct Standing {
    stands: bool,
    about: Option<String>,
    reasons: BTreeMap<String, String>,
    answer: Option<u64>,
}

fn standing(work: &Work, words: &AskProtected) -> Option<Standing> {
    let all = questions(&work.records);
    let q = all.get(words.name.as_str()).filter(|q| q.latest.engine)?;
    let (about, reasons) = match work.question(q.latest) {
        Some(Question::Asked { about, reasons, .. }) => (about.clone(), reasons.clone()),
        _ => (None, BTreeMap::new()),
    };
    Some(Standing {
        stands: q.stands(),
        about,
        reasons,
        answer: q.answer.map(|a| a.option),
    })
}

impl Standing {
    /// Whether it stands of the file as it is: its latest version is of the file with the content it now ends with.
    fn of(&self, file: &File) -> bool {
        let line = file.line();
        self.stands && (self.about.as_deref()).is_some_and(|about| about.lines().any(|l| l == line))
    }
}

/// Why the work changes each file of the change, by its path: as `given` says it, a return's `protected_changes`,
/// where it says it; else as the question that stands says it of the file as it is. A file of which neither says, or
/// says nothing but whitespace, has none.
fn reasons(standing: Option<&Standing>, change: &Change, given: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let said = |why: &&String| !why.trim().is_empty();
    let mut reasons = BTreeMap::new();
    for file in &change.files {
        let returned = given.get(&file.path).filter(said);
        let standing_says = (standing.filter(|s| s.of(file)))
            .and_then(|s| s.reasons.get(&file.path))
            .filter(said);
        if let Some(why) = returned.or(standing_says) {
            reasons.insert(file.path.clone(), why.trim().to_string());
        }
    }
    reasons
}

/// The path of each file of the change that has no reason ([`reasons`]) and that the owner did not let merge as it is
/// without one, by the question that stands of it, in the change's order: what the work is still to say why of.
fn unexplained(
    standing: Option<&Standing>,
    change: &Change,
    reasons: &BTreeMap<String, String>,
    merges: u64,
) -> Vec<String> {
    let merges_as_it_is = |file: &File| standing.is_some_and(|s| s.of(file) && s.answer == Some(merges));
    (change.files.iter())
        .filter(|file| !reasons.contains_key(&file.path) && !merges_as_it_is(file))
        .map(|file| file.path.clone())
        .collect()
}

/// Whether the owner let this change merge: the engine's question stands of this very change, and its latest version
/// was answered with the option `merges` names.
pub(crate) fn let_merge(work: &Work, words: &AskProtected, change: &Change) -> bool {
    let key = change.key();
    standing(work, words)
        .is_some_and(|s| s.stands && s.about.as_deref() == Some(key.as_str()) && s.answer == Some(words.merges))
}

/// What the engine asks of the change, where it is to ask anything, each file with why the work changes it as `given`
/// or the question that stands says it ([`reasons`]): the question asked, or asked anew where the one that stands is of
/// another change, or says other reasons and is not answered, or was answered with another option than the one that
/// lets it merge; or withdrawn where the work's change is empty and one stands. None where the question stands
/// unanswered of this change with these reasons, or was answered letting this change merge, whatever reasons are given
/// for it since.
pub(crate) fn asking(
    work: &Work,
    words: &AskProtected,
    change: &Change,
    given: &BTreeMap<String, String>,
) -> Option<Question> {
    let standing = standing(work, words);
    asked(standing.as_ref(), words, change, given)
}

fn asked(
    standing: Option<&Standing>,
    words: &AskProtected,
    change: &Change,
    given: &BTreeMap<String, String>,
) -> Option<Question> {
    if change.files.is_empty() {
        return standing.filter(|s| s.stands).map(|_| Question::Withdrawn {
            name: words.name.clone(),
            why: words.withdrawn.clone(),
        });
    }
    let reasons = reasons(standing, change, given);
    let key = change.key();
    let of_it = standing.filter(|s| s.stands && s.about.as_deref() == Some(key.as_str()));
    let stays = of_it.is_some_and(|s| s.answer == Some(words.merges) || (s.answer.is_none() && s.reasons == reasons));
    if stays {
        return None;
    }
    let files: Vec<String> = (change.files.iter())
        .map(|file| {
            let kind = match file.status {
                'A' => &words.added,
                'D' => &words.deleted,
                _ => &words.changed,
            };
            let why = reasons.get(&file.path).unwrap_or(&words.no_reason);
            let places = [
                ("path", &file.path),
                ("change", kind),
                ("why", why),
                ("diff", &file.diff),
            ];
            fill(&words.file, &places.map(|(name, value)| (name, value.as_str())))
        })
        .collect();
    let ask = fill(&words.ask, &[("files", &files.join("\n\n"))]);
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
        about: Some(key),
        reasons,
    })
}

/// What the engine finds of the work's change under the protected paths as a round ends.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Looked {
    /// What it asks the owner, where it asks anything.
    pub(crate) asks: Option<Question>,
    /// The path of each file of the change of which neither the return nor the question that stands says why, and that
    /// the owner did not let merge as it is, where the return does the cell's work, which is sent back for them.
    pub(crate) unexplained: Vec<String>,
}

/// What the engine finds of the work's change under the protected paths at the head of its branch as a round ends,
/// the round's return saying why the work changes each file where it does the cell's work (`given`), none where it
/// only asks for calls: the early notice of what the check before the merge holds the work to. Where a file of the
/// change is unexplained ([`unexplained`]), nothing is asked, and a return that does the cell's work is sent back for
/// each such file; one that asks for calls cannot say why, and the round of its run that does the cell's work is held
/// to it.
pub(crate) async fn look(
    project: &Project,
    place: &Place,
    main: &str,
    paths: &[String],
    words: &AskProtected,
    work: &Work,
    given: Option<&BTreeMap<String, String>>,
) -> Result<Looked, String> {
    let change = change_at(project, place, main, &place.reference(), paths).await?;
    let none = BTreeMap::new();
    let said = given.unwrap_or(&none);
    let standing = standing(work, words);
    let reasons = reasons(standing.as_ref(), &change, said);
    let unexplained = unexplained(standing.as_ref(), &change, &reasons, words.merges);
    if !unexplained.is_empty() {
        let unexplained = if given.is_some() { unexplained } else { Vec::new() };
        return Ok(Looked {
            asks: None,
            unexplained,
        });
    }
    Ok(Looked {
        asks: asked(standing.as_ref(), words, &change, said),
        unexplained: Vec::new(),
    })
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

    /// Why a return says the work changes each of these files, by its path.
    fn because(said: &[(&str, &str)]) -> BTreeMap<String, String> {
        (said.iter())
            .map(|(path, why)| (path.to_string(), why.to_string()))
            .collect()
    }

    /// A reason for each file the tests change under the protected paths.
    fn every_reason() -> BTreeMap<String, String> {
        because(&[
            ("AGENTS.md", "The guide must say it."),
            ("flow/cell.toml", "The flow needs the cell."),
        ])
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

    /// What the engine finds as a round ends with the work's branch as it is, the round's return saying this of why,
    /// none where it only asks for calls.
    async fn found(
        repo: &Repo,
        place: &Place,
        work: &Work,
        given: Option<&BTreeMap<String, String>>,
    ) -> Result<Looked, String> {
        look(&repo.project, place, "main", &paths(), &words(), work, given).await
    }

    /// What the engine asks as a round ends with the work's branch as it is, its return saying why of every file the
    /// tests change, so that nothing is unexplained.
    async fn looked(repo: &Repo, place: &Place, work: &Work) -> Result<Option<Question>, String> {
        let looked = found(repo, place, work, Some(&every_reason())).await?;
        assert_eq!(looked.unexplained, Vec::<String>::new(), "every file has its reason");
        Ok(looked.asks)
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
        let nothing = found(&repo, &place, &Work::default(), Some(&BTreeMap::new())).await;
        assert_eq!(nothing, Ok(Looked::default()));
    }

    #[tokio::test]
    async fn a_change_that_touches_a_protected_path_is_asked_of_the_owner_with_each_file_s_reason_above_its_change() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        commits(&place.dir, "greeting.txt", "Hello, you.\n");
        let Ok(Some(Question::Asked {
            name,
            waits,
            recommends,
            ask,
            options,
            about,
            reasons,
        })) = looked(&repo, &place, &Work::default()).await
        else {
            panic!("nothing is asked");
        };
        assert_eq!(name, "protected_files");
        assert!(waits, "the work waits for the answer");
        assert_eq!(recommends, None, "the engine recommends nothing");
        // Each file, with its reason in the work's words, and then its own change.
        let (guide, cell) = (
            ask.find("- AGENTS.md: added. Why: The guide must say it.\n")
                .expect(&ask),
            ask.find("- flow/cell.toml: added. Why: The flow needs the cell.\n")
                .expect(&ask),
        );
        assert!(ask.starts_with("May this merge?\n\n- AGENTS.md: "), "{ask}");
        let (rules, title) = (
            ask.find("+Rules.").expect(&ask),
            ask.find("+title = \"A cell\"").expect(&ask),
        );
        assert!(guide < rules && rules < cell && cell < title, "{ask}");
        assert!(!ask.contains("greeting.txt"), "only what is protected is shown: {ask}");
        assert_eq!(options.len(), 2);
        assert_eq!(reasons, every_reason(), "the question keeps why of each file");
        let about = about.expect("the question says what change it is of");
        let files: Vec<&str> = about.lines().map(|line| line.split(' ').next().unwrap()).collect();
        assert_eq!(files, ["AGENTS.md", "flow/cell.toml"], "{about}");
    }

    #[tokio::test]
    async fn a_return_that_changes_a_protected_file_and_says_not_why_is_sent_back_for_it_and_nothing_is_asked() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let work = Work::default();
        let none = found(&repo, &place, &work, Some(&BTreeMap::new())).await.unwrap();
        assert_eq!(none.asks, None, "nothing is asked before the work says why");
        assert_eq!(none.unexplained, ["AGENTS.md", "flow/cell.toml"]);
        // A reason of one file leaves the other unexplained, and a reason of nothing but whitespace is none.
        let said = because(&[("AGENTS.md", "The guide must say it."), ("flow/cell.toml", "  \n")]);
        let one = found(&repo, &place, &work, Some(&said)).await.unwrap();
        assert_eq!((one.asks, one.unexplained), (None, vec!["flow/cell.toml".to_string()]));
        // A reason of a file the work does not change explains nothing.
        let elsewhere = because(&[
            ("flow/other.toml", "Not changed."),
            ("AGENTS.md", "The guide must say it."),
        ]);
        let other = found(&repo, &place, &work, Some(&elsewhere)).await.unwrap();
        assert_eq!(other.unexplained, ["flow/cell.toml"]);
        // A return that only asks for calls cannot say why, and is not sent back for it; nothing is asked either.
        assert_eq!(found(&repo, &place, &work, None).await, Ok(Looked::default()));
    }

    #[tokio::test]
    async fn a_change_taken_back_in_the_work_asks_nothing_and_needs_no_reason() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        run(&place.dir, &["rm", "-q", "flow/cell.toml"]);
        run(&place.dir, &["commit", "-q", "-m", "Take the cell back"]);
        let nothing = found(&repo, &place, &Work::default(), Some(&BTreeMap::new())).await;
        assert_eq!(nothing, Ok(Looked::default()));
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
    async fn a_reason_stands_for_the_file_as_it_is_and_a_file_changed_again_needs_its_own() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let first = because(&[("AGENTS.md", "The guide must say it.")]);
        let asked = found(&repo, &place, &Work::default(), Some(&first))
            .await
            .unwrap()
            .asks
            .unwrap();
        for answer in [None, Some(words().merges)] {
            let work = asked_with(asked.clone(), answer);
            // A later round that says nothing of the file as it is, or says the same, asks nothing.
            assert_eq!(
                found(&repo, &place, &work, Some(&BTreeMap::new())).await,
                Ok(Looked::default())
            );
            assert_eq!(found(&repo, &place, &work, Some(&first)).await, Ok(Looked::default()));
        }
        // Changed again, the file needs a reason of its own, and is asked anew with it.
        let work = asked_with(asked.clone(), Some(words().merges));
        commits(&place.dir, "AGENTS.md", "Other rules.\n");
        let none = found(&repo, &place, &work, Some(&BTreeMap::new())).await.unwrap();
        assert_eq!((none.asks, none.unexplained), (None, vec!["AGENTS.md".to_string()]));
        let second = because(&[("AGENTS.md", "The rules changed again.")]);
        let anew = found(&repo, &place, &work, Some(&second)).await.unwrap();
        let Some(Question::Asked { ask, reasons, .. }) = anew.asks else {
            panic!("the change made again is not asked: {anew:?}");
        };
        assert!(ask.contains("Why: The rules changed again.\n"), "{ask}");
        assert!(!ask.contains("The guide must say it."), "{ask}");
        assert_eq!(reasons, second);
    }

    // Another reason for the file as it is asks anew while the question waits, and lets nothing merge before the
    // answer; the owner's yes to the change holds whatever reason a round gives after it, so that the same change is
    // not asked again. A yes given to a question with no reason was asked again for the reason a later round gave
    // (work 9).
    #[tokio::test]
    async fn another_reason_asks_anew_while_the_question_waits_and_a_yes_holds_whatever_reason_comes_after_it() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let first = because(&[("AGENTS.md", "The guide must say it.")]);
        let asked = found(&repo, &place, &Work::default(), Some(&first))
            .await
            .unwrap()
            .asks
            .unwrap();
        let other = because(&[("AGENTS.md", "The guide must say it, and why.")]);
        let waiting = asked_with(asked.clone(), None);
        let anew = found(&repo, &place, &waiting, Some(&other)).await.unwrap().asks;
        let Some(question @ Question::Asked { .. }) = anew else {
            panic!("another reason is not asked: {anew:?}");
        };
        assert!(
            matches!(&question, Question::Asked { ask, .. } if ask.contains("Why: The guide must say it, and why."))
        );
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        assert!(
            !let_merge(&asked_with(question, None), &words(), &change),
            "asked anew, it waits for an answer"
        );
        let let_it = asked_with(asked, Some(words().merges));
        assert_eq!(found(&repo, &place, &let_it, Some(&other)).await, Ok(Looked::default()));
        assert!(let_merge(&let_it, &words(), &change));
        // The check's question says no reason: a reason a round gives after the owner's yes to it asks nothing.
        let unexplained = asking(&Work::default(), &words(), &change, &BTreeMap::new()).unwrap();
        let let_it = asked_with(unexplained, Some(words().merges));
        assert_eq!(found(&repo, &place, &let_it, Some(&first)).await, Ok(Looked::default()));
    }

    #[tokio::test]
    async fn the_check_asks_with_the_reasons_the_question_says_and_says_a_file_no_round_explained_has_none() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        let Some(Question::Asked { ask, reasons, .. }) = asking(&Work::default(), &words(), &change, &BTreeMap::new())
        else {
            panic!("nothing is asked");
        };
        assert!(ask.contains("- AGENTS.md: added. Why: none.\ndiff --git"), "{ask}");
        assert!(reasons.is_empty());
        // The owner's yes to it as it is needs no reason after it: a later round is not sent back, and nothing is asked.
        let backstop = asking(&Work::default(), &words(), &change, &BTreeMap::new()).unwrap();
        let let_it = asked_with(backstop.clone(), Some(words().merges));
        assert_eq!(
            found(&repo, &place, &let_it, Some(&BTreeMap::new())).await,
            Ok(Looked::default())
        );
        // Any other answer leaves the work to say why, or take it back.
        let other = if words().merges == 1 { 2 } else { 1 };
        let no = found(
            &repo,
            &place,
            &asked_with(backstop, Some(other)),
            Some(&BTreeMap::new()),
        )
        .await;
        assert_eq!(no.unwrap().unexplained, ["AGENTS.md"]);
        // Of a change the question that stands says why, the check asks again, where it must, with that reason.
        let first = because(&[("AGENTS.md", "The guide must say it.")]);
        let asked = found(&repo, &place, &Work::default(), Some(&first))
            .await
            .unwrap()
            .asks
            .unwrap();
        let other = if words().merges == 1 { 2 } else { 1 };
        let taken_back = asked_with(asked, Some(other));
        let Some(Question::Asked { ask, .. }) = asking(&taken_back, &words(), &change, &BTreeMap::new()) else {
            panic!("the change is not asked anew");
        };
        assert!(
            ask.contains("- AGENTS.md: added. Why: The guide must say it.\n"),
            "{ask}"
        );
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
        let none = found(&repo, &place, &work, Some(&BTreeMap::new())).await;
        assert_eq!(
            none,
            Ok(Looked::default()),
            "main's change is no change of the work's to explain"
        );
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        let files: Vec<&str> = change.files.iter().map(|file| file.path.as_str()).collect();
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
        let withdrawn = found(&repo, &place, &work, Some(&BTreeMap::new())).await;
        assert_eq!(
            withdrawn,
            Ok(Looked {
                asks: Some(Question::Withdrawn {
                    name: "protected_files".to_string(),
                    why: words().withdrawn,
                }),
                unexplained: Vec::new(),
            })
        );
        let work = asked_with(withdrawn.unwrap().asks.unwrap(), None);
        assert_eq!(looked(&repo, &place, &work).await, Ok(None));
        assert_eq!(looked(&repo, &place, &Work::default()).await, Ok(None));
        // Made again as it was, the change is not explained by the question withdrawn.
        commits(&place.dir, "flow/cell.toml", "title = \"A cell\"\n");
        let again = found(&repo, &place, &work, Some(&BTreeMap::new())).await.unwrap();
        assert_eq!(again.unexplained, ["flow/cell.toml"]);
    }

    #[tokio::test]
    async fn only_the_option_that_lets_it_merge_lets_this_very_change_merge_and_any_other_asks_anew() {
        let (repo, place) = started().await;
        commits(&place.dir, "AGENTS.md", "Rules.\n");
        let asked = looked(&repo, &place, &Work::default()).await.unwrap().unwrap();
        let head = place.reference();
        let change = change_at(&repo.project, &place, "main", &head, &paths()).await.unwrap();
        let (words, none) = (words(), BTreeMap::new());
        let unanswered = asked_with(asked.clone(), None);
        assert!(!let_merge(&unanswered, &words, &change), "not answered");
        assert_eq!(
            asking(&unanswered, &words, &change, &none),
            None,
            "it stands unanswered"
        );
        let let_it = asked_with(asked.clone(), Some(words.merges));
        assert!(let_merge(&let_it, &words, &change));
        assert_eq!(asking(&let_it, &words, &change, &none), None);
        let other = if words.merges == 1 { 2 } else { 1 };
        let taken_back = asked_with(asked, Some(other));
        assert!(!let_merge(&taken_back, &words, &change), "another option is no yes");
        let anew = asking(&taken_back, &words, &change, &none);
        let key = change.key();
        assert!(
            matches!(&anew, Some(Question::Asked { about, .. }) if about.as_deref() == Some(key.as_str())),
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
        let asked = look(
            &repo.project,
            &place,
            "main",
            &[],
            &words(),
            &Work::default(),
            Some(&BTreeMap::new()),
        )
        .await;
        assert_eq!(asked, Ok(Looked::default()));
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

    #[tokio::test]
    async fn a_path_that_reads_as_a_pattern_is_shown_as_the_file_it_is() {
        let (repo, place) = started().await;
        commits(&place.dir, "flow/a*.toml", "star = true\n");
        commits(&place.dir, "flow/ab.toml", "plain = true\n");
        let said = because(&[("flow/a*.toml", "A star."), ("flow/ab.toml", "Plain.")]);
        let asked = found(&repo, &place, &Work::default(), Some(&said)).await.unwrap().asks;
        let Some(Question::Asked { ask, .. }) = asked else {
            panic!("nothing is asked: {asked:?}");
        };
        let (star, plain) = (
            ask.find("- flow/a*.toml: added").expect(&ask),
            ask.find("- flow/ab.toml").expect(&ask),
        );
        let shown = &ask[star..plain];
        assert!(
            shown.contains("+star = true") && !shown.contains("+plain = true"),
            "{ask}"
        );
    }
}
