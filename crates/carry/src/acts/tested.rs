//! The act that runs the tests the cells ask for (`run_tests`): of each cell that asks in a record the act's cell is
//! run by and that applies, its entry that waits for its result ([`decide::asked_of`]), run on the work's branch as the
//! host runs the project's tests, its result handed in with the round's end as the cell's own block that the result
//! comes in, which runs it again (decision a-wait-holds-its-cell). What a newer entry of the same cell replaced has
//! what it asked the host for cancelled, and what cannot be cancelled is kept as an observation. A red test is what the
//! run found, and what could not be run is said in the result once and not run again; the round fails only where the
//! host could not be reached, which waits with every act until it can be.
//!
//! An entry may name, by its full hash, a commit the work's branch reaches, which is run in place of the last commit or
//! the worktree (decision an-entry-names-the-commit-it-runs-on): checked as text before any git command runs, then
//! found by git as that very commit, then reached from the branch's own reference; anything else is said once, as what
//! could not be run. A host that runs the tests in a working tree runs it in a worktree of its own beside the work's
//! ([`git::at_commit`]), made with the repository held and taken away when the round is stopped. Each result says
//! first the entry it answers.

use decide::{BlockId, CellId};
use flow::Compiled;
use flow::agents::RunTests;
use serde::Deserialize;
use work::{Handed, Work};

use tokio::sync::Mutex;

use super::conflict;
use super::failing;
use crate::git;
use crate::held::Stop;
use crate::host::{Asking, Branch, Failed, Host, Named};
use crate::settings::Project;

/// What a round of the act hands in: the result of each entry, as the asking cell's block of it; and what it came
/// across beyond the work, as a cancel the host refused.
pub(crate) struct Tested {
    pub results: Vec<Handed>,
    pub observations: Vec<String>,
}

/// Runs, for the cell `giving` of the flow, each entry it gives the result of, and gives each result as the asking
/// cell's block of it, its first line the entry it answers; first cancels what the entries replaced asked for, saying in
/// `words` what could not be. None where it was told to stop; an `Err` is a host that could not be reached. The
/// repository is held only while a worktree of named commits is made or taken away.
pub(crate) async fn act(
    host: &impl Host,
    (project, words, repository): (&Project, &RunTests, &Mutex<()>),
    branch: &Branch<'_>,
    (flow, work, giving): (&Compiled, &Work, &CellId),
    stop: &mut Stop,
) -> Result<Option<Tested>, Failed> {
    let entries = decide::asked_of(&flow.flow, &work.records, giving);
    let content =
        |record: &BlockId, at: decide::Seq| work.contents.get(&(record.clone(), at)).map_or("", String::as_str);
    let replaced: Vec<(Asking, Option<String>)> = (entries.iter())
        .flat_map(|entry| {
            let commit = |at| named(content(&entry.record, at)).ok().and_then(|n| n.commit);
            (entry.replaced.iter()).map(move |at| (Asking::Entry(*at), commit(*at)))
        })
        .collect();
    let observations = cancel(host, branch, &replaced, words).await;
    let mut results = Vec::new();
    for entry in &entries {
        let named = named(content(&entry.record, entry.at));
        let said = match ran(
            host,
            (project, words, repository),
            branch,
            named,
            Asking::Entry(entry.at),
            stop,
        )
        .await
        {
            Ok(Some(said)) => said,
            Ok(None) => return Ok(None),
            // A host out of reach is no answer to the entry: the round waits with every act for the host.
            Err(failed) if failed.kind.is_none() && failing::network(&failed.why) => return Err(failed),
            Err(failed) => could_not(words, &failed.why),
        };
        let first = flow::agents::fill(&words.entry, &[("entry", &entry.at.0.to_string())]);
        results.push(Handed {
            block: entry.result.clone(),
            content: format!("{first}\n{said}"),
            matters: true,
            value: None,
        });
    }
    Ok(Some(Tested { results, observations }))
}

/// What a run of the entry, as it was read, says: why it is no entry, or why the commit it names is none of the work's
/// branch, in the words of what could not be run; or what the host found, of the commit named in a worktree of its own
/// where the host runs the tests in one, which is taken away where the round was stopped. None where it was stopped.
async fn ran(
    host: &impl Host,
    (project, words, repository): (&Project, &RunTests, &Mutex<()>),
    branch: &Branch<'_>,
    named: Result<Named, String>,
    asking: Asking,
    stop: &mut Stop,
) -> Result<Option<String>, Failed> {
    let named = match named {
        Ok(named) => named,
        Err(why) => return Ok(Some(could_not(words, &why))),
    };
    let Some(commit) = named.commit.as_deref() else {
        return run(host, project, branch, &named, asking, stop).await;
    };
    if !of_the_branch(project, branch.place, commit).await? {
        return Ok(Some(could_not(words, &words.not_of_the_branch)));
    }
    if !host.runs_in_a_worktree() {
        return run(host, project, branch, &named, asking, stop).await;
    }
    let at = {
        let _held = repository.lock().await;
        git::at_commit(project, branch.place, commit).await?
    };
    let on = Branch { place: &at, ..*branch };
    let said = run(host, project, &on, &named, asking, stop).await?;
    if said.is_none() {
        let _held = repository.lock().await;
        git::remove(project, &at).await?;
    }
    Ok(said)
}

/// Whether the text names a commit by its full hash, 40 or 64 lowercase hexadecimal characters: read as text, before
/// any git command is given it, so that an agent's text is never an argument it can steer.
fn full_hash(text: &str) -> bool {
    (text.len() == 40 || text.len() == 64) && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether the text names a commit of the work's branch: a full hash ([`full_hash`]), which git finds as that very
/// commit (a tag's hash peels to another, and is none), and which the branch's own reference reaches, never the
/// worktree's head, which its agent may move.
async fn of_the_branch(project: &Project, place: &git::Place, text: &str) -> Result<bool, String> {
    if !full_hash(text) {
        return Ok(false);
    }
    git::configuration(project, place).await?;
    let repo = &project.repo;
    let peeled = format!("{text}^{{commit}}");
    let found = git::said(repo, &["rev-parse", "--verify", "--quiet", "--end-of-options", &peeled]).await?;
    if !found.ok || found.out.trim() != text {
        return Ok(false);
    }
    let reached = git::said(repo, &["merge-base", "--is-ancestor", text, &place.reference()]).await?;
    match reached.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor {text} {}: {}",
            place.reference(),
            reached.err.trim()
        )),
    }
}

/// What asked the host for the whole of the project's checks for the work: each entry in which a cell asked for tests
/// and named none, so that the full workflows ran, the latest first. The check before the merge takes their passing on
/// the very commit it checks as its own (decision the-check-takes-a-full-run-that-passed).
pub(crate) fn whole(flow: &Compiled, work: &Work) -> Vec<Asking> {
    let records: Vec<&BlockId> = (flow.flow.cells().iter())
        .flat_map(|cell| cell.asks.iter().map(|asks| &asks.record))
        .collect();
    let names_none = |block: &BlockId, at: decide::Seq| {
        (work.contents.get(&(block.clone(), at))).is_some_and(|entry| named(entry).is_ok_and(|n| n.is_empty()))
    };
    (work.records.finished.iter().rev())
        .filter(|f| (f.writes.iter()).any(|w| records.contains(&&w.block) && names_none(&w.block, f.at)))
        .map(|f| Asking::Entry(f.at))
        .collect()
}

/// Cancels what the replaced entries asked the host for. What a replaced entry still runs costs a runner, and nothing
/// reads it: what cannot be cancelled is kept as an observation, in the words of the act's file (`not_cancelled`), and
/// the entries are run all the same. Nothing is kept where nothing was replaced, or where it was cancelled.
async fn cancel(
    host: &impl Host,
    branch: &Branch<'_>,
    replaced: &[(Asking, Option<String>)],
    words: &RunTests,
) -> Vec<String> {
    if replaced.is_empty() {
        return Vec::new();
    }
    let Err(failed) = host.cancel_tests(branch, replaced).await else {
        return Vec::new();
    };
    let names: Vec<String> = replaced.iter().map(|(asking, _)| asking.name()).collect();
    let entries = names.join(", ");
    vec![flow::agents::fill(
        &words.not_cancelled,
        &[("entries", &entries), ("why", &failed.why)],
    )]
}

/// What a result says where the tests could not be run, said once, in the words of the act's file (`could_not`): the
/// cell may ask again.
fn could_not(words: &RunTests, why: &str) -> String {
    flow::agents::fill(&words.could_not, &[("why", why)])
}

/// Runs the tests and checks as the host does, unless the work's branch does not merge into main's head as the host
/// last gave it, which is said at once.
pub(crate) async fn run(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    named: &Named,
    asking: Asking,
    stop: &mut Stop,
) -> Result<Option<String>, Failed> {
    if let Some(said) = conflict::found(project, branch.place, &host.kept_main()).await? {
        return Ok(Some(said));
    }
    host.run_tests(branch, named, asking, stop).await
}

/// An entry of `run_tests`: the tests and the checks it names, each list left out where it names none, and the commit
/// it names to run them on, where it names one.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
    #[serde(default)]
    commit: Option<String>,
}

/// What an entry names, each name trimmed and taken once, a blank one left out; an entry with nothing in it names
/// nothing. Or why it is no entry, as the reading says it.
pub(crate) fn named(input: &str) -> Result<Named, String> {
    if input.trim().is_empty() {
        return Ok(Named::default());
    }
    let entry: Entry = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let once = |names: Vec<String>| {
        let mut taken: Vec<String> = Vec::new();
        for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            if !taken.iter().any(|t| t == name) {
                taken.push(name.to_string());
            }
        }
        taken
    };
    Ok(Named {
        tests: once(entry.tests),
        checks: once(entry.checks),
        commit: entry.commit,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use store::WorkNumber;

    use super::*;
    use crate::git;
    use crate::git::tests::Repo;
    use crate::held::Held;
    use crate::host::Local;
    use crate::host::fake::{Asked, Fake};

    const WORK: WorkNumber = WorkNumber(1);

    /// The local host, saying what a run of the tests says as this repository's file beside the flows has it.
    fn local(repo: &Repo) -> Local {
        let words = crate::acts::words().run_tests;
        Local::new(repo.project.clone()).saying(Some(&words))
    }

    /// What a run of these tests says on this host.
    async fn said_on(host: &impl Host, repo: &Repo, named: &Named) -> String {
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        match run(
            host,
            &repo.project,
            &branch,
            named,
            Asking::Entry(decide::Seq(1)),
            &mut Stop::never(),
        )
        .await
        {
            Ok(Some(said)) => said,
            Ok(None) => panic!("nothing stopped it"),
            Err(failed) => could_not(&crate::acts::words().run_tests, &failed.why),
        }
    }

    #[tokio::test]
    async fn the_tests_say_whether_they_passed_with_what_they_said() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let head = crate::git::tests::run(&repo.place(WORK).dir, &["rev-parse", "HEAD"]);
        assert_eq!(
            said_on(&local(&repo), &repo, &Named::default()).await,
            format!("On the commit {head}:\nThe check `echo all is well` passed. What it said:\n```\nall is well\n```")
        );
        let failing = Repo::new("echo it broke; exit 2");
        git::start(&failing.project, &failing.place(WORK), "main")
            .await
            .unwrap();
        let said = said_on(&local(&failing), &failing, &Named::default()).await;
        let (_, said) = said.split_once('\n').expect("the commit first");
        assert!(said.starts_with("The check `echo it broke; exit 2` failed ("), "{said}");
        assert!(said.ends_with("```\nit broke\n```"), "{said}");
    }

    // What the check said is set apart from the words around it, whatever it holds: a heading or a fence of its own
    // stands within a fence longer than any of its own, and comes back whole. Well under a second alone; the check's
    // output on GitHub is held to the same by the tests of its host.
    #[tokio::test]
    async fn what_the_check_said_is_set_apart_whatever_it_holds() {
        let repo = Repo::new("printf '# A heading of its own\\n```\\nits fence\\n```\\n'");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        let given = "What it said:\n````\n# A heading of its own\n```\nits fence\n```\n````";
        assert!(said.ends_with(given), "{said}");
    }

    #[tokio::test]
    async fn what_the_tests_said_is_given_whole_to_be_kept_whole() {
        let repo = Repo::new("seq 1 3000");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        assert!(
            said.contains("\n1\n2\n3\n") && said.ends_with("\n2999\n3000\n```"),
            "{said}"
        );
    }

    #[tokio::test]
    async fn the_tests_are_given_of_the_engines_environment_only_what_the_agents_are() {
        // Cargo gives the tests' process this variable, which no agent is given unless the settings name it: the
        // token for GitHub is one such.
        assert!(std::env::var("CARGO_PKG_NAME").is_ok(), "cargo runs the tests");
        let repo = Repo::new("env");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        assert!(said.contains("\nHOME=") || said.contains(":\nHOME="), "{said}");
        assert!(!said.contains("CARGO_PKG_NAME="), "{said}");
        let giving = local(&repo).giving(vec!["CARGO_PKG_NAME".to_string()]);
        let named = said_on(&giving, &repo, &Named::default()).await;
        assert!(named.contains("CARGO_PKG_NAME=carry"), "{named}");
    }

    #[tokio::test]
    async fn tests_with_no_worktree_say_so() {
        let repo = Repo::new("true");
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        assert!(
            said.starts_with("The check `true` could not be run: the work has no worktree"),
            "{said}"
        );
    }

    #[tokio::test]
    async fn stopped_tests_are_stopped_where_they_run() {
        let repo = Repo::new("sleep 30");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let began = Instant::now();
        let stopping = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(held.stop(1, ()).is_none(), "what runs it is told to stop");
        };
        let host = local(&repo);
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        let named = Named::default();
        let ran = run(
            &host,
            &repo.project,
            &branch,
            &named,
            Asking::Entry(decide::Seq(1)),
            &mut stop,
        );
        let (ran, ()) = tokio::join!(ran, stopping);
        assert!(matches!(ran, Ok(None)), "{ran:?}");
        assert!(began.elapsed() < Duration::from_secs(20), "the command was stopped");
    }

    #[test]
    fn an_entry_names_tests_and_checks_each_once_or_nothing_for_the_whole() {
        let input = r#"{"tests": ["tools::tests::a", "  src/x.test.ts > a describe > its title ", "tools::tests::a", " "],
                        "checks": ["lint"]}"#;
        assert_eq!(
            named(input).expect("read"),
            Named {
                tests: vec![
                    "tools::tests::a".to_string(),
                    "src/x.test.ts > a describe > its title".to_string()
                ],
                checks: vec!["lint".to_string()],
                commit: None,
            }
        );
        for whole in ["", "  ", "{}", r#"{"tests": [], "checks": []}"#] {
            assert_eq!(named(whole).unwrap(), Named::default(), "{whole:?}");
        }
        for wrong in [r#"{"test": ["a"]}"#, r#"{"tests": "a"}"#, "tools::tests::a"] {
            assert!(named(wrong).is_err(), "{wrong:?}");
        }
    }

    #[tokio::test]
    async fn the_host_is_asked_for_what_the_entry_names_by_the_entry_and_what_it_says_is_the_result() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        crate::git::tests::commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let host = Fake::new("main").runs("ran what was named");
        let named = Named {
            tests: vec!["a".to_string()],
            checks: vec!["lint".to_string()],
            commit: None,
        };
        assert_eq!(said_on(&host, &repo, &named).await, "ran what was named");
        assert_eq!(said_on(&host, &repo, &Named::default()).await, "ran what was named");
        assert_eq!(
            host.asked(),
            [Asked::RunTests(named), Asked::RunTests(Named::default())]
        );
        assert_eq!(host.askings(), [Asking::Entry(decide::Seq(1)); 2]);
    }

    // What the host could not cancel of what the entries a newer one replaced asked it for is kept as one observation,
    // in the words of the act's file, and nothing is kept where it cancelled or where nothing was replaced. Well under
    // a millisecond alone, its repository made and never run; no other test holds what a failed cancel leaves.
    #[tokio::test]
    async fn what_the_host_could_not_cancel_is_kept_in_the_words_of_the_acts_file_and_nothing_else_is() {
        let repo = Repo::new("true");
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        let words = crate::acts::words().run_tests;
        let replaced = [
            (Asking::Entry(decide::Seq(1)), None),
            (Asking::Entry(decide::Seq(4)), Some("c0ffee".repeat(6) + "c0ff")),
        ];
        let host = Fake::new("main").refuses_cancel("the runs are gone");
        let kept = cancel(&host, &branch, &replaced, &words).await;
        let opening = words.not_cancelled.split('{').next().expect("words");
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(kept[0].starts_with(opening), "{kept:?}");
        assert!(kept[0].contains(&Asking::Entry(decide::Seq(4)).name()), "{kept:?}");
        assert!(
            kept[0].contains("the runs are gone") && !kept[0].contains('{'),
            "{kept:?}"
        );
        assert_eq!(host.asked(), [Asked::Cancel(replaced.to_vec())]);
        let host = Fake::new("main");
        assert_eq!(cancel(&host, &branch, &replaced, &words).await, Vec::<String>::new());
        assert_eq!(cancel(&host, &branch, &[], &words).await, Vec::<String>::new());
        assert_eq!(host.asked(), [Asked::Cancel(replaced.to_vec())]);
    }

    /// The branch of the work in the repository, its place this.
    fn branch_of(place: &git::Place) -> Branch<'_> {
        Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place,
        }
    }

    /// What a run of the entry naming this commit says on the local host, as the act runs it.
    async fn on_commit(repo: &Repo, commit: Option<&str>, stop: &mut Stop) -> Result<Option<String>, Failed> {
        let words = crate::acts::words().run_tests;
        let place = repo.place(WORK);
        let named = Named {
            commit: commit.map(str::to_string),
            ..Named::default()
        };
        let lock = Mutex::new(());
        let with = (&repo.project, &words, &lock);
        ran(
            &local(repo),
            with,
            &branch_of(&place),
            Ok(named),
            Asking::Entry(decide::Seq(1)),
            stop,
        )
        .await
    }

    // A commit is named by its full hash alone, 40 or 64 lowercase hexadecimal characters, read as text before git is
    // given it: a revision git would read another way, a short or upper-case hash, or one that begins as an option, is
    // none. Well under a millisecond alone; no other test holds the text an agent may name a commit by.
    #[test]
    fn a_commit_is_named_by_its_full_hash_alone() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        assert!(full_hash(full));
        assert!(full_hash(&full.repeat(2)[..64]));
        let refused = [
            "HEAD~1".to_string(),
            "@{1}".to_string(),
            ":/text".to_string(),
            "main".to_string(),
            full[..12].to_string(),
            full.to_uppercase(),
            format!("-{}", &full[1..]),
            format!("{full}0"),
            String::new(),
        ];
        for text in refused {
            assert!(!full_hash(&text), "{text}");
        }
    }

    // An entry runs the commit it names where that is a commit the work's branch reaches, by its full hash: on the local
    // host in a worktree of its own beside the work's, which is reused from entry to entry and leaves the work's own
    // worktree as it is; each run says first the commit it ran on. Anything else is run nowhere: a commit of another
    // branch, a tag's hash, a hash git does not know, and a name that is no full hash, each said once. Under a second
    // alone, a repository and two worktrees made; no other test holds what an entry may name or where it runs.
    #[tokio::test]
    async fn an_entry_runs_a_commit_of_the_branch_it_names_beside_the_works_worktree_and_no_other() {
        let repo = Repo::new("git rev-parse HEAD");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let fork = crate::git::tests::run(&place.dir, &["rev-parse", "HEAD"]);
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests, the change not made.\n");
        let change = crate::git::tests::commit(&place.dir, "change.txt", "The change.\n");
        std::fs::write(place.dir.join("draft.txt"), "Not committed.\n").unwrap();
        let mut stop = Stop::never();
        let commits = git::commits_place(&place).dir;
        for named in [&tests, &fork, &change] {
            let said = on_commit(&repo, Some(named), &mut stop).await.unwrap().expect("run");
            assert!(said.starts_with(&format!("On the commit {named}:\n")), "{said}");
            assert!(said.ends_with(&format!("\n{named}\n```")), "{said}");
            assert_eq!(commits.parent(), place.dir.parent(), "beside the work's worktree");
            assert!(!commits.starts_with(&place.dir), "never in it");
        }
        let status = crate::git::tests::run(&commits, &["status", "--porcelain"]);
        assert_eq!(status, "", "the worktree of commits holds the commit alone");
        assert_eq!(crate::git::tests::run(&place.dir, &["rev-parse", "HEAD"]), change);
        assert!(place.dir.join("draft.txt").exists(), "the work's worktree is as it was");
        let said = on_commit(&repo, None, &mut stop).await.unwrap().expect("run");
        assert!(
            said.starts_with(&format!(
                "On the working tree at {change}, with changes not committed:\n"
            )),
            "{said}"
        );
        // Another branch's commit, a tag's hash, a hash git does not know, and what is no full hash.
        let main = &repo.project.repo;
        crate::git::tests::run(main, &["checkout", "-q", "-b", "other"]);
        let other = crate::git::tests::commit(main, "other.txt", "Another branch.\n");
        crate::git::tests::run(main, &["checkout", "-q", "main"]);
        crate::git::tests::run(main, &["tag", "-a", "v1", "-m", "A tag.", &tests]);
        let tag = crate::git::tests::run(main, &["rev-parse", "v1"]);
        assert_ne!(tag, tests, "a tag's own hash");
        let words = crate::acts::words().run_tests;
        let nowhere = could_not(&words, &words.not_of_the_branch);
        let unknown = "0".repeat(40);
        let short = tests[..12].to_string();
        for named in [
            other.as_str(),
            &tag,
            &unknown,
            &short,
            "HEAD~1",
            "main",
            &tests.to_uppercase(),
        ] {
            let said = on_commit(&repo, Some(named), &mut stop).await.unwrap();
            assert_eq!(said.as_deref(), Some(nowhere.as_str()), "{named}");
        }
    }

    // A round stopped while it runs a named commit takes the worktree of commits away, whatever its checkout was doing;
    // and what is at that worktree's place that is a symbolic link, which the engine did not make, is left as it is
    // and runs nothing. Under a second alone, the command stopped at once; no other test holds them.
    #[tokio::test]
    async fn a_stopped_run_of_a_named_commit_takes_its_worktree_away_and_a_link_at_its_place_is_left() {
        let repo = Repo::new("sleep 30");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests.\n");
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let stopping = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(held.stop(1, ()).is_none(), "what runs it is told to stop");
        };
        let (ran, ()) = tokio::join!(on_commit(&repo, Some(&tests), &mut stop), stopping);
        assert!(matches!(ran, Ok(None)), "{ran:?}");
        let commits = git::commits_place(&place).dir;
        assert!(!commits.exists(), "taken away");
        let elsewhere = repo.data.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &commits).unwrap();
        let why = on_commit(&repo, Some(&tests), &mut Stop::never())
            .await
            .expect_err("refused");
        assert!(why.why.contains("is a symbolic link"), "{why:?}");
        assert!(git::a_link(&commits), "left as it is");
    }

    // From one run of a named commit to the next, the worktree of commits loses what is in it untracked and keeps what
    // the repository ignores, where a build keeps its output warm. Under a second alone, a repository and two
    // worktrees; no other test holds what the worktree of commits keeps.
    #[tokio::test]
    async fn the_worktree_of_commits_loses_what_is_untracked_and_keeps_what_is_ignored() {
        let repo = Repo::new("true");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let ignoring = crate::git::tests::commit(&place.dir, ".gitignore", "target/\n");
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests.\n");
        let mut stop = Stop::never();
        on_commit(&repo, Some(&ignoring), &mut stop)
            .await
            .unwrap()
            .expect("run");
        let commits = git::commits_place(&place).dir;
        std::fs::write(commits.join("left.txt"), "Untracked.\n").unwrap();
        std::fs::create_dir_all(commits.join("target")).unwrap();
        std::fs::write(commits.join("target/warm.txt"), "Built.\n").unwrap();
        on_commit(&repo, Some(&tests), &mut stop).await.unwrap().expect("run");
        assert!(!commits.join("left.txt").exists(), "what is untracked is taken away");
        assert!(commits.join("target/warm.txt").exists(), "what is ignored stays");
    }

    // A worktree of commits whose index a checkout killed with the engine left locked is made again, once, and the
    // entry runs on it. Under a second alone, a repository and two worktrees; no other test holds a worktree of commits
    // made again.
    #[tokio::test]
    async fn a_worktree_of_commits_left_locked_is_made_again_and_the_entry_runs() {
        let repo = Repo::new("git rev-parse HEAD");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests.\n");
        let change = crate::git::tests::commit(&place.dir, "change.txt", "The change.\n");
        let mut stop = Stop::never();
        on_commit(&repo, Some(&tests), &mut stop).await.unwrap().expect("run");
        let commits = git::commits_place(&place).dir;
        let path = ["rev-parse", "--path-format=absolute", "--git-path", "index.lock"];
        let lock = crate::git::tests::run(&commits, &path);
        std::fs::write(&lock, "").unwrap();
        let said = on_commit(&repo, Some(&change), &mut stop).await.unwrap().expect("run");
        assert!(said.starts_with(&format!("On the commit {change}:\n")), "{said}");
        assert!(said.ends_with(&format!("\n{change}\n```")), "{said}");
        assert!(!std::path::Path::new(&lock).exists(), "made again");
    }

    // A plain directory at the place of the worktree of commits, which git does not know, is left as it is, what it
    // holds and all, and runs nothing. Well under a second alone; no other test holds a directory the engine did not
    // make at that place.
    #[tokio::test]
    async fn a_plain_directory_at_the_place_of_the_worktree_of_commits_is_left_as_it_is() {
        let repo = Repo::new("true");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests.\n");
        let commits = git::commits_place(&place).dir;
        std::fs::create_dir_all(&commits).unwrap();
        std::fs::write(commits.join("kept.txt"), "A person's.\n").unwrap();
        let why = on_commit(&repo, Some(&tests), &mut Stop::never())
            .await
            .expect_err("refused");
        assert!(why.why.contains("is no worktree of the repository"), "{why:?}");
        let kept = std::fs::read_to_string(commits.join("kept.txt")).expect("left");
        assert_eq!(kept, "A person's.\n");
        assert_eq!(std::fs::read_dir(&commits).unwrap().count(), 1, "nothing is added");
    }

    // An entry that named the point the branch left main at, or a commit of its own, is run the same after the branch
    // takes main in, as a round of the act started again runs it: the branch still reaches each, though the first is
    // none of its commits since main. Under a second alone, a repository and two worktrees; no other test holds an
    // entry to a branch that took main in after it.
    #[tokio::test]
    async fn an_entry_is_run_the_same_after_the_branch_takes_main_in() {
        let repo = Repo::new("git rev-parse HEAD");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let fork = crate::git::tests::run(&place.dir, &["rev-parse", "HEAD"]);
        let tests = crate::git::tests::commit(&place.dir, "tests.txt", "The tests.\n");
        let mut stop = Stop::never();
        let ran = |said: &str, named: &str| said.starts_with(&format!("On the commit {named}:\n"));
        for named in [&fork, &tests] {
            let said = on_commit(&repo, Some(named), &mut stop).await.unwrap().expect("run");
            assert!(ran(&said, named), "{said}");
        }
        let main = &repo.project.repo;
        crate::git::tests::commit(main, "farewell.txt", "Bye.\n");
        crate::git::tests::run(&place.dir, &["merge", "--quiet", "--no-edit", "main"]);
        let since = crate::git::tests::run(main, &["rev-list", &format!("main..{}", place.reference())]);
        assert!(!since.contains(&fork) && since.contains(&tests), "{since}");
        for named in [&fork, &tests] {
            let said = on_commit(&repo, Some(named), &mut stop).await.unwrap().expect("run");
            assert!(ran(&said, named), "{said}");
        }
    }

    // The tests of a worktree in which a merge is under way are refused, as every act is, while the repository's
    // configuration names a program git could run, before git looks at the worktree or the command runs: the look at
    // whether the branch merges into main lets a merge under way through unread. Under a second alone, a repository and
    // its worktree; no other test holds the local host's run to the repository's configuration.
    #[tokio::test]
    async fn the_tests_of_a_worktree_mid_merge_are_refused_where_the_configuration_names_a_program() {
        let repo = Repo::new("touch ran.txt");
        let place = repo.place(WORK);
        git::start(&repo.project, &place, "main").await.unwrap();
        let main = &repo.project.repo;
        crate::git::tests::commit(main, "farewell.txt", "Bye.\n");
        crate::git::tests::run(&place.dir, &["merge", "--quiet", "--no-commit", "--no-ff", "main"]);
        crate::git::tests::run(main, &["config", "core.fsmonitor", "/bin/true"]);
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        assert!(
            said.starts_with("The check `touch ran.txt` could not be run: "),
            "{said}"
        );
        assert!(said.contains("core.fsmonitor"), "{said}");
        assert!(!place.dir.join("ran.txt").exists(), "the command ran");
    }

    // The runs the check may take as its own are those of the entries of `run_tests` that named no tests or checks, the
    // latest first. Well under a millisecond alone; no other test holds the order of them.
    #[test]
    fn the_whole_runs_are_those_of_the_entries_that_named_nothing_the_latest_first() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&[
                "answer",
                "asked",
                "dependencies",
                "main",
                "session",
                "start",
                "head",
                "tree",
                "words",
            ]),
            acts: names(&["start", "check", "merge", "deliver", "run_tests"]),
            tools: names(&["query"]),
            details: names(&["files"]),
            ..Default::default()
        };
        let source = flow::read(&root.join("engineer")).expect("the flow is read");
        let flow = flow::compile(&source, &has).expect("the flow compiles");
        let mut work = Work::default();
        let mut at = Vec::new();
        for (run, entry) in [
            (1, "{}"),
            (2, r#"{"tests": ["a"]}"#),
            (3, r#"{"checks": ["lint"]}"#),
            (4, ""),
        ] {
            let seq = work.tick();
            work.records.finished.push(decide::Finished {
                run: decide::RunId(run),
                at: seq,
                writes: vec![decide::Write {
                    block: "run_tests".into(),
                    meaning_unchanged: false,
                    empty: false,
                    value: None,
                }],
                dealt_with: Some(Vec::new()),
                waits_for: None,
            });
            work.contents.insert(("run_tests".into(), seq), entry.to_string());
            at.push(seq);
        }
        assert_eq!(whole(&flow, &work), [Asking::Entry(at[3]), Asking::Entry(at[0])]);
    }

    // An entry names its commit as a string; one that is no string is no entry, said by its reading. Well under a
    // millisecond alone; no other test holds the reading of an entry's commit.
    #[test]
    fn an_entry_reads_the_commit_it_names_as_a_string() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        let read = named(&format!(r#"{{"tests": ["a"], "commit": "{full}"}}"#)).expect("read");
        assert_eq!(read.commit.as_deref(), Some(full));
        let why = named(r#"{"commit": 1}"#).expect_err("no entry");
        assert!(why.contains("invalid type: integer"), "{why}");
    }

    #[tokio::test]
    async fn tests_that_could_not_be_run_say_why_once_and_are_not_tried_again() {
        let repo = Repo::new("true");
        let host = Fake::new("main");
        let said = said_on(&host, &repo, &Named::default()).await;
        let why = crate::host::fake::NO_TOOLS;
        let words = crate::acts::words().run_tests.could_not;
        assert_eq!(said, flow::agents::fill(&words, &[("why", why)]));
        assert!(said.ends_with(why) && !said.contains('{'), "{said}");
        assert_eq!(host.asked(), [Asked::RunTests(Named::default())]);
    }
}
