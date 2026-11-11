//! The work's branch taking main in as a run of the build, which takes main in, starts.

use carry::ROUNDS;
use service::Act;

use crate::{Fake, On, Real, ended, git_in};

/// A check that fails the first time, moving main by a commit of `other.txt` as it does, and passes after.
const MOVES_MAIN_ONCE: &str = "test -f ../moved || { touch ../moved; \
    r=$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\"); \
    printf 'Good day.\\n' > \"$r/other.txt\"; git -C \"$r\" add other.txt; \
    git -C \"$r\" -c user.name=Tester -c user.email=tester@example.com commit -q -m 'Main moved'; exit 1; }";

#[tokio::test]
async fn a_run_of_the_build_after_main_moved_takes_it_in_and_is_told_so_once() {
    let mut real = Real::open(On::Acts, Fake::default(), None, MOVES_MAIN_ONCE);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(
        real.finished(work),
        ["start", "build", "check", "build", "check", "merge"]
    );
    let kept = real.work(work);
    let builds: Vec<_> = (kept.records.started.iter())
        .filter(|s| s.cell.as_str() == "build")
        .map(|s| s.run)
        .collect();
    let rounds = real.data.join(ROUNDS).join(work.0.to_string());
    let told = |run: decide::RunId| {
        std::fs::read_to_string(rounds.join(format!("{}-1.message.md", run.0))).expect("the round was told")
    };
    // The first build's branch held main, as the start made it: it is told nothing of it.
    assert!(!told(builds[0]).contains("taken in"), "{}", told(builds[0]));
    // Main moved during the first check; the second build took it in before its first round, and was told so.
    let second = told(builds[1]);
    assert!(
        second.contains(&format!(
            "\n\n## main taken in\nThe engine merged main at {} into the work's branch as this run started",
            git_in(&real.repo, &["log", "-1", "--format=%H", "--grep=Main moved", "main"])
        )),
        "{second}"
    );
    assert!(
        second.contains("since the branch last took it in:\nother.txt\n"),
        "{second}"
    );
    // What came of it is kept for the run, once.
    for run in &builds {
        assert!(rounds.join(format!("{}.main.json", run.0)).is_file());
    }
    // The branch holds the engine's merge of main, and the work merged it.
    let place = real.place(work);
    let log = git_in(&real.repo, &["log", &place.reference(), "--format=%s %ae"]);
    assert!(
        log.lines()
            .any(|l| l.starts_with("Merge main at ") && l.ends_with(" wf-agents@localhost")),
        "{log}"
    );
}
