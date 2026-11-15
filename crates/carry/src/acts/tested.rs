//! The act that runs the tests the cells ask for (`run_tests`): of each cell that asks in a record the act's cell is
//! run by, its latest entry with no result yet ([`decide::asked_of`]), run on the work's branch as the host runs the
//! project's tests, its result written into the cell's own block that the result comes in, which runs it again
//! (decision a-wait-holds-its-cell). What a newer entry of the same cell replaced has what it asked the host for
//! cancelled. A red test is what the run found, and what could not be run is said in the result once and not run
//! again; the round fails only where the host could not be reached, which waits with every act until it can be.

use decide::{BlockId, CellId};
use flow::Compiled;
use serde::Deserialize;
use service::Teller;
use work::Work;

use super::conflict;
use super::failing;
use crate::held::Stop;
use crate::host::{Asking, Branch, Failed, Host, Named};
use crate::settings::Project;

/// Runs, for the cell `giving` of the flow, each entry it gives the result of, and writes each result as the source
/// outside the work that writes the asking cell's block of it; first cancels what the entries replaced asked for.
/// Says false where it was told to stop, having written nothing; an `Err` is a host that could not be reached.
pub(crate) async fn act(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    (flow, work, giving): (&Compiled, &Work, &CellId),
    teller: &Teller,
    stop: &mut Stop,
) -> Result<bool, Failed> {
    let entries = decide::asked_of(&flow.flow, &work.records, giving);
    let replaced: Vec<Asking> = (entries.iter())
        .flat_map(|entry| entry.replaced.iter().map(|at| Asking::Entry(*at)))
        .collect();
    if !replaced.is_empty()
        && let Err(failed) = host.cancel_tests(branch, &replaced).await
    {
        // What a replaced entry still runs costs a runner, and nothing reads it: what cannot be cancelled is said, and
        // the entries are run all the same.
        eprintln!(
            "work {}: what replaced entries of run_tests asked for could not be cancelled: {}",
            branch.work.0, failed.why
        );
    }
    let mut results: Vec<(BlockId, String)> = Vec::new();
    for entry in &entries {
        let input = (work.contents.get(&(entry.record.clone(), entry.at))).map_or("", String::as_str);
        let said = match named(input) {
            Err(why) => could_not(&why),
            Ok(named) => match run(host, project, branch, &named, Asking::Entry(entry.at), stop).await {
                Ok(Some(said)) => said,
                Ok(None) => return Ok(false),
                // A host out of reach is no answer to the entry: the round waits with every act for the host.
                Err(failed) if failed.kind.is_none() && failing::network(&failed.why) => return Err(failed),
                Err(failed) => could_not(&failed.why),
            },
        };
        results.push((entry.result.clone(), said));
    }
    for (block, said) in results {
        teller.outside_wrote(block, said);
    }
    Ok(true)
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

/// What a result says where the tests could not be run, said once: the cell may ask again.
fn could_not(why: &str) -> String {
    format!("The check could not be run: {why}")
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

/// An entry of `run_tests`: the tests and the checks it names, each list left out where it names none.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
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
            Err(failed) => could_not(&failed.why),
        }
    }

    #[tokio::test]
    async fn the_tests_say_whether_they_passed_with_what_they_said() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        assert_eq!(
            said_on(&local(&repo), &repo, &Named::default()).await,
            "The check `echo all is well` passed. What it said:\nall is well\n"
        );
        let failing = Repo::new("echo it broke; exit 2");
        git::start(&failing.project, &failing.place(WORK), "main")
            .await
            .unwrap();
        let said = said_on(&local(&failing), &failing, &Named::default()).await;
        assert!(said.starts_with("The check `echo it broke; exit 2` failed ("), "{said}");
        assert!(said.ends_with("it broke\n"), "{said}");
    }

    #[tokio::test]
    async fn what_the_tests_said_is_given_whole_to_be_kept_whole() {
        let repo = Repo::new("seq 1 3000");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = said_on(&local(&repo), &repo, &Named::default()).await;
        assert!(
            said.contains("\n1\n2\n3\n") && said.ends_with("\n2999\n3000\n"),
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
        };
        assert_eq!(said_on(&host, &repo, &named).await, "ran what was named");
        assert_eq!(said_on(&host, &repo, &Named::default()).await, "ran what was named");
        assert_eq!(
            host.asked(),
            [Asked::RunTests(named), Asked::RunTests(Named::default())]
        );
        assert_eq!(host.askings(), [Asking::Entry(decide::Seq(1)); 2]);
    }

    #[tokio::test]
    async fn tests_that_could_not_be_run_say_why_once_and_are_not_tried_again() {
        let repo = Repo::new("true");
        let host = Fake::new("main");
        let said = said_on(&host, &repo, &Named::default()).await;
        let why = crate::host::fake::NO_TOOLS;
        assert_eq!(said, format!("The check could not be run: {why}"));
        assert_eq!(host.asked(), [Asked::RunTests(Named::default())]);
    }
}
