//! A run that waits for a runner: it waits on while a runner of its kind is online, busy or not, and is cancelled where
//! none is or it cannot be told; the time it may take counts only the time it does not wait; and it says the runner it
//! waits for. Each wait here is real time, with the window a run may wait before the runners are asked after shortened
//! to a tenth of a second.

use super::super::api::{Layer, Outcome, Runner};
use super::super::fast::Waiting;
use super::super::looks::Looks;
use super::fast::{junit_green, named};
use super::*;

/// The window a run may wait for a runner before the runners are asked after, in these tests.
const WINDOW: Duration = Duration::from_millis(100);

/// The hub with the project's workflows run on the runner `14900k`, which the installation has.
pub(super) fn on_14900k(hub: Hub) -> Hub {
    hub.with(|config| {
        config.start = WINDOW;
        config.runners = vec!["github".to_string(), "14900k".to_string()];
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast.runner = "14900k".to_string();
        ci.full.runner = "14900k".to_string();
    })
}

fn runner(online: bool, labels: &[&str]) -> Runner {
    Runner {
        online,
        labels: labels.iter().map(|label| label.to_string()).collect(),
    }
}

/// A runner that is online, busy as far as the engine knows or not, under a label that differs in case from the
/// project's.
fn online() -> Layer {
    Ok(vec![runner(true, &["self-hosted", "14900K"])])
}

fn refused() -> Layer {
    Err("HTTP 403: Resource not accessible by personal access token".to_string())
}

/// A request of the fast workflow for the test `tools::tests::a`, made to the hub's host.
async fn asking(hub: &Hub) -> Result<Option<String>, String> {
    hub.named_check(&named(&["tools::tests::a"], &[])).await
}

// A run that no runner has taken waits on while a runner of its kind is online, busy or not, however long it waits, and
// is read as any other once it has run: the engine asks again after each window, which this one does more than once
// before the run starts. The runner's label differs in case from the project's.
#[tokio::test]
async fn a_run_waits_on_for_a_busy_runner_that_is_online_and_is_read_once_it_has_run() {
    let hub = on_14900k(Hub::new());
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![passed("test")], Some(("test-results", junit_green())));
    hub.github().lists_runners(online(), None);
    hub.github().queue_runs();
    let starting = async {
        tokio::time::sleep(Duration::from_millis(450)).await;
        hub.github().start_runs();
    };
    let (said, ()) = tokio::join!(asking(&hub), starting);
    let said = said.unwrap().expect("nothing stopped it");
    assert!(said.contains("1 of the 1 tests passed"), "{said}");
    assert!(cancelled(&hub).is_empty(), "no run was cancelled");
    assert!(
        hub.github().runners_asked() >= 2,
        "the runners were asked after at each window: {}",
        hub.github().runners_asked()
    );
}

// A run waits where the layer of the runners that lists an online one is read, though the other layer is refused: a
// token that reads the organisation's runners but not the repository's has the runners that are there.
#[tokio::test]
async fn a_run_waits_on_where_one_layer_lists_an_online_runner_though_the_other_is_refused() {
    let hub = on_14900k(Hub::new());
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![passed("test")], Some(("test-results", junit_green())));
    hub.github().lists_runners(refused(), Some(online()));
    hub.github().queue_runs();
    let starting = async {
        tokio::time::sleep(Duration::from_millis(350)).await;
        hub.github().start_runs();
    };
    let (said, ()) = tokio::join!(asking(&hub), starting);
    assert!(said.unwrap().expect("nothing stopped it").contains("1 of the 1 tests"));
    assert!(cancelled(&hub).is_empty());
}

// A run that waits where no runner of its kind is online is cancelled once it has waited the window, and its words say
// which kind: none listed, one that is offline, and one online under another label are all none of its kind.
#[tokio::test]
async fn a_run_that_waits_where_no_runner_of_its_kind_is_online_is_cancelled_saying_which_kind() {
    let listed: [Layer; 3] = [
        Ok(Vec::new()),
        Ok(vec![runner(false, &["self-hosted", "14900k"])]),
        Ok(vec![runner(true, &["self-hosted", "ubuntu"])]),
    ];
    for runners in listed {
        let hub = on_14900k(Hub::new());
        let head = hub.committed().await;
        hub.github().lists_runners(runners.clone(), Some(Ok(Vec::new())));
        hub.github().queue_runs();
        let why = asking(&hub).await.expect_err("not run");
        assert!(
            why.starts_with(&format!(
                "the run of fast.yml asked for on {head} never started: no runner took it after "
            )),
            "{runners:?}: {why}"
        );
        assert!(
            why.ends_with(
                "(https://github.example/actions/runs/1000), none of 14900k is online, and the engine cancelled the run"
            ),
            "{runners:?}: {why}"
        );
        assert_eq!(cancelled(&hub), [1000], "{runners:?}");
    }
}

// Where no layer lists an online runner and one could not be read, the engine cannot tell that none is online: the run is
// cancelled, and its words say so, with GitHub's own, and do not say that none is.
#[tokio::test]
async fn a_run_is_cancelled_saying_it_could_not_be_told_where_the_runners_cannot_be_read() {
    let hub = on_14900k(Hub::new());
    hub.committed().await;
    hub.github().lists_runners(refused(), Some(refused()));
    hub.github().queue_runs();
    let why = asking(&hub).await.expect_err("not run");
    assert!(
        why.contains(
            "the engine could not tell whether a runner of 14900k is online (HTTP 403: Resource not accessible"
        ),
        "{why}"
    );
    assert!(!why.contains("none of 14900k is online"), "{why}");
    assert_eq!(cancelled(&hub), [1000]);
    // A layer read that lists none, beside one refused, is no more known: the refused one may list one.
    let hub = on_14900k(Hub::new());
    hub.committed().await;
    hub.github().lists_runners(Ok(Vec::new()), Some(refused()));
    hub.github().queue_runs();
    let why = asking(&hub).await.expect_err("not run");
    assert!(
        why.contains("could not tell whether a runner of 14900k is online"),
        "{why}"
    );
}

// GitHub's own machines are never offline: a run on them that waits is waited for, and the runners are not asked after.
#[tokio::test]
async fn a_run_on_githubs_own_machines_waits_on_and_the_runners_are_not_asked_after() {
    let hub = Hub::new().with(|config| config.start = WINDOW);
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![passed("test")], Some(("test-results", junit_green())));
    hub.github().queue_runs();
    let starting = async {
        tokio::time::sleep(Duration::from_millis(350)).await;
        hub.github().start_runs();
    };
    let (said, ()) = tokio::join!(asking(&hub), starting);
    assert!(said.unwrap().expect("nothing stopped it").contains("1 of the 1 tests"));
    assert!(cancelled(&hub).is_empty());
    assert_eq!(hub.github().runners_asked(), 0);
}

// A run that has started but whose jobs all wait for a runner is waiting for it, and its time, which a run may take, does
// not run while it does: it waits longer than that time with no job under way and is not cut for it. It says the runner it waits for from the first look, before the
// runners are asked after, and none once a job is under way; and its jobs are looked at by one look at a time, which
// GitHub counts only as the jobs change.
#[tokio::test]
async fn a_started_run_whose_jobs_wait_for_a_runner_is_not_timed_while_they_wait_and_says_which() {
    let hub = on_14900k(Hub::new()).with(|config| {
        config.ci.as_mut().expect("a declaration").fast.too_long = "1s".to_string();
        config.start = Duration::from_millis(300);
    });
    let head = hub.committed().await;
    let results = || Some(("test-results", junit_green()));
    hub.github()
        .fast(&head, vec![check("test", Outcome::Queued)], results());
    hub.github().lists_runners(online(), None);
    hub.github().runs_go_on();
    let (to, mut came) = tokio::sync::mpsc::unbounded_channel();
    let reply = service::CallReply::new(WORK, CallId(2), to);
    let saying = reply.saying();
    let held: Held<u8, service::CallReply> = Held::new();
    let mut stop = held.hold(1, reply).expect("held").saying(saying);
    let along = |came: &mut tokio::sync::mpsc::UnboundedReceiver<service::Message>| {
        let mut said = Vec::new();
        while let Ok(message) = came.try_recv() {
            if let service::Message::Along { along, .. } = message {
                said.push(along);
            }
        }
        said
    };
    let (place, asked) = (hub.place(), named(&["tools::tests::a"], &[]));
    let (branch, call) = (branch(&place), hub.call());
    let run = "https://github.example/actions/runs/1000".to_string();
    let watching = async {
        // Before the window the run waits, and the runners are not yet asked after: it is said at the first look.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(hub.github().runners_asked(), 0);
        let said = along(&mut came);
        assert_eq!(
            said.last(),
            Some(&Some(service::Along::Runs {
                at: run.clone(),
                queued_for: Some("14900k".to_string())
            })),
            "{said:?}"
        );
        // Past the time a run may take, with no job under way: it is not cut for its time.
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(cancelled(&hub).is_empty(), "it was cut for its time while it waited");
        hub.github()
            .fast(&head, vec![check("test", Outcome::Pending)], results());
        tokio::time::sleep(Duration::from_millis(150)).await;
        hub.github().fast(&head, vec![passed("test")], results());
        hub.github().finish_runs();
        came
    };
    let (said, mut came) = tokio::join!(hub.host.run_tests(&branch, &asked, call, &mut stop), watching);
    let said = said.unwrap().expect("nothing stopped it");
    assert!(said.contains("1 of the 1 tests passed"), "{said}");
    assert!(!said.contains("did not complete within"), "{said}");
    assert!(cancelled(&hub).is_empty());
    let said = along(&mut came);
    assert_eq!(
        said.last(),
        Some(&Some(service::Along::Runs {
            at: run,
            queued_for: None
        })),
        "{said:?}"
    );
    let looks = hub.github().jobs_looked();
    let counted = looks.iter().filter(|look| look.counted).count();
    assert!(looks.len() > 100, "it waited for its jobs: {} looks", looks.len());
    assert!(counted <= 4, "{counted} of {} looks were counted", looks.len());
}

// A started run whose jobs wait for a runner where none of its kind is online is cancelled once it has waited the window,
// as one no runner took is, its words saying that a job of it never started.
#[tokio::test]
async fn a_started_run_whose_jobs_wait_where_no_runner_is_online_is_cancelled() {
    let hub = on_14900k(Hub::new());
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![check("test", Outcome::Queued)],
        Some(("test-results", junit_green())),
    );
    hub.github().lists_runners(Ok(Vec::new()), None);
    hub.github().runs_go_on();
    let why = asking(&hub).await.expect_err("not run");
    assert!(
        why.starts_with(&format!(
            "a job of the run of fast.yml asked for on {head} never started: no runner took its jobs after "
        )),
        "{why}"
    );
    assert!(why.contains("none of 14900k is online"), "{why}");
    assert_eq!(cancelled(&hub), [1000]);
}

// A run that waits and is then taken by a runner is timed from then on: a run queued for longer than the time a run may
// take, and then started, is not cut for time at once, as one that had run all that time would be.
#[tokio::test]
async fn a_run_queued_longer_than_the_time_it_may_take_is_not_cut_for_it_once_it_starts() {
    let hub = on_14900k(Hub::new()).with(|config| {
        config.ci.as_mut().expect("a declaration").fast.too_long = "1s".to_string();
        config.start = Duration::from_millis(400);
    });
    let head = hub.committed().await;
    let results = || Some(("test-results", junit_green()));
    hub.github()
        .fast(&head, vec![check("test", Outcome::Pending)], results());
    hub.github().lists_runners(online(), None);
    hub.github().runs_go_on();
    hub.github().queue_runs();
    let starting = async {
        tokio::time::sleep(Duration::from_millis(1100)).await;
        hub.github().start_runs();
        tokio::time::sleep(Duration::from_millis(200)).await;
        hub.github().fast(&head, vec![passed("test")], results());
        hub.github().finish_runs();
    };
    let (said, ()) = tokio::join!(asking(&hub), starting);
    let said = said.unwrap().expect("nothing stopped it");
    assert!(said.contains("1 of the 1 tests passed"), "{said}");
    assert!(!said.contains("did not complete within"), "{said}");
    assert!(cancelled(&hub).is_empty());
}

// What a run is timed by is the time it did not wait for a runner, which a look sets by what the last look found: the
// stretch since the last look is the time it did not wait where it did not wait at that look, and the window the
// runners are asked after is its own, which starts anew each time the run stops waiting.
#[test]
fn a_run_is_timed_only_by_the_time_it_does_not_wait_for_a_runner() {
    let began = tokio::time::Instant::now();
    let at = |seconds: u64| began + Duration::from_secs(seconds);
    let window = Duration::from_secs(30);
    let mut waiting = Waiting::new(began);
    waiting.tick(at(10), false);
    assert_eq!(waiting.ran_for, Duration::from_secs(10));
    assert!(
        !waiting.due(at(10), window),
        "a run that does not wait is not asked after"
    );
    // It waits from the look at 20: the stretch before it, which did not wait at the look before, is its time.
    waiting.tick(at(20), true);
    assert_eq!(waiting.ran_for, Duration::from_secs(20));
    assert!(!waiting.due(at(49), window));
    assert!(waiting.due(at(50), window));
    // Asked after at 50 and one online: it is asked after again a window later, and what it waited is not its time.
    waiting.tick(at(50), true);
    waiting.asked(at(50), window);
    assert_eq!(waiting.ran_for, Duration::from_secs(20));
    assert!(!waiting.due(at(79), window));
    assert!(waiting.due(at(80), window));
    // A job under way: the stretch that held it is still waiting's, the next is the run's.
    waiting.tick(at(90), false);
    assert_eq!(waiting.ran_for, Duration::from_secs(20));
    waiting.tick(at(100), false);
    assert_eq!(waiting.ran_for, Duration::from_secs(30));
    assert!(
        !waiting.due(at(200), window),
        "what it waited is forgotten once it does not wait"
    );
    // It waits again: the window begins anew.
    waiting.tick(at(110), true);
    assert!(!waiting.due(at(139), window));
    assert!(waiting.due(at(140), window));
}

// The jobs of a run are looked at by one look at a time however many waits ask: an answer young enough is given to each
// as it is, a look sent now carries the tag of the last answer and GitHub answers it unchanged where nothing changed,
// which costs nothing, and a look after the run is let go is a first look again.
#[tokio::test]
async fn the_waits_on_a_runs_jobs_share_one_look_and_an_unchanged_answer_costs_nothing() {
    use super::super::api::Api;
    let hub = Hub::new();
    let jobs = vec![passed("lint"), check("test", Outcome::Queued)];
    hub.github().fast("c0ffee", jobs.clone(), None);
    let inputs = [
        ("commit".to_string(), "c0ffee".to_string()),
        ("call".to_string(), "x".to_string()),
    ];
    hub.github()
        .dispatch("fast.yml", "main", &inputs)
        .await
        .expect("a run by hand");
    let (looks, api, hour) = (Looks::default(), hub.github(), Duration::from_secs(3600));
    let counted = || hub.github().jobs_looked().iter().filter(|look| look.counted).count();
    let (a, b, c) = tokio::join!(
        looks.jobs(api, 1000, hour),
        looks.jobs(api, 1000, hour),
        looks.jobs(api, 1000, hour)
    );
    for read in [a, b, c] {
        assert_eq!(read.expect("the jobs"), jobs);
    }
    assert_eq!(hub.github().jobs_looked().len(), 1, "three waits, one look");
    let again = looks.jobs(api, 1000, Duration::ZERO).await.expect("the jobs");
    assert_eq!(again, jobs, "unchanged gives the jobs of the answer before");
    assert_eq!(
        (hub.github().jobs_looked().len(), counted()),
        (2, 1),
        "the look sent now was not counted"
    );
    let changed = vec![passed("lint"), passed("test")];
    hub.github().fast("c0ffee", changed.clone(), None);
    assert_eq!(looks.jobs(api, 1000, Duration::ZERO).await.expect("the jobs"), changed);
    assert_eq!(
        (hub.github().jobs_looked().len(), counted()),
        (3, 2),
        "a change is counted"
    );
    looks.forget(1000);
    looks.jobs(api, 1000, hour).await.expect("the jobs");
    assert_eq!(
        (hub.github().jobs_looked().len(), counted()),
        (4, 3),
        "a look after it was let go is a first"
    );
}
