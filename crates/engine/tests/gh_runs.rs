//! How the engine looks at a workflow's runs, over a stand-in for the real gh command: what it asks gh for, what it sends
//! back, and what it does with the answers. The stand-in answers from files a test lays out and logs each command line.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use engine::hub::{Gh, GitHub};

/// The stand-in for gh. It finds the repository in the command line (`-R owner/name`, or the path of `gh api`), logs the
/// command line as tab-separated words, counts how many commands of each kind it was asked, and answers the n-th of a
/// kind from the files `<kind>-<n>.out` (stdout; `.old` for the listing of `gh run list`), `.err` (stderr) and `.code`
/// (exit status, 0 if none). A kind it has no file for for the n-th time gets exit status 9.
const FAKE_GH: &str = r#"#!/bin/bash
repo=""
prev=""
for a in "$@"; do
  case "$a" in
    repos/*) repo=$(echo "${a#repos/}" | cut -d/ -f1,2 | tr / _) ;;
  esac
  if [ "$prev" = "-R" ]; then repo=$(echo "$a" | tr / _); fi
  prev="$a"
done
dir="$WF_FAKE_GH_DIR/$repo"
printf '%s\t' "$@" >> "$dir/log"
echo >> "$dir/log"
kind=""
case "$1" in
  api)
    case "$2" in
      *"/actions/workflows?"*) kind=workflows ;;
      *"/runs?"*) kind=runs ;;
    esac ;;
  run)
    kind="$2"
    [ "$kind" = list ] && kind=runs ;;
esac
n=$(cat "$dir/$kind.count" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$dir/$kind.count"
f="$dir/$kind-$n"
ext=out
[ "$1" = run ] && [ "$kind" = runs ] && ext=old
if [ ! -f "$f.$ext" ]; then
  echo "the stand-in has no answer for $kind $n" >&2
  exit 9
fi
[ -f "$f.err" ] && cat "$f.err" >&2
cat "$f.$ext"
exit "$(cat "$f.code" 2>/dev/null || echo 0)"
"#;

/// Runs `body` in a copy of this test process that finds the stand-in first on its PATH, and gives it the directory of
/// the stand-in's files; `setup` lays those files out first, in this process. The copy is the same test run again, since
/// the PATH of a process cannot be changed from inside it. It fails unless the copy ran `body` to its end.
fn with_fake_gh(test: &str, setup: impl FnOnce(&Path), body: impl FnOnce(&Path)) {
    if let Some(dir) = std::env::var_os("WF_FAKE_GH_DIR") {
        let dir = PathBuf::from(dir);
        body(&dir);
        fs::write(dir.join("passed"), "").unwrap();
        return;
    }
    let dir = std::env::temp_dir().join(format!("wf-agents-fake-gh-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    let script = dir.join("bin").join("gh");
    fs::write(&script, FAKE_GH).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    setup(&dir);
    let path = format!(
        "{}:{}",
        dir.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env("PATH", path)
        .env("WF_FAKE_GH_DIR", &dir)
        .output()
        .unwrap();
    assert!(
        out.status.success() && dir.join("passed").exists(),
        "{test} failed in the copy that has the stand-in for gh:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The directory of the stand-in's files for one repository.
fn repo_dir(root: &Path, repo: &str) -> PathBuf {
    let dir = root.join(repo.replace('/', "_"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A run as the answers of both listings give it: `(id, status, conclusion, attempt)`.
type Run = (i64, &'static str, Option<&'static str>, i64);

/// What the API of runs prints for `gh api <path> -i`: the status line ends in a line feed alone, every header in a
/// carriage return and a line feed, then a blank line and the body (the form of a capture of 2026-11-04).
fn api_200(etag: Option<&str>, runs: &[Run]) -> String {
    let runs: Vec<String> = runs
        .iter()
        .map(|(id, status, conclusion, attempt)| {
            serde_json::json!({
                "id": id, "status": status, "conclusion": conclusion, "run_attempt": attempt,
                "html_url": format!("https://github.com/o/r/actions/runs/{id}"),
                "url": format!("https://api.github.com/repos/o/r/actions/runs/{id}"),
            })
            .to_string()
        })
        .collect();
    let tag = etag.map(|t| format!("Etag: {t}\r\n")).unwrap_or_default();
    format!(
        "HTTP/2.0 200 OK\nContent-Type: application/json; charset=utf-8\r\n{tag}X-Ratelimit-Used: 7\r\n\r\n{{\"total_count\":{},\"workflow_runs\":[{}]}}",
        runs.len(),
        runs.join(",")
    )
}

/// What the listing of `gh run list --json databaseId,status,conclusion,url,attempt` prints.
fn run_list(runs: &[Run]) -> String {
    let runs: Vec<String> = runs
        .iter()
        .map(|(id, status, conclusion, attempt)| {
            serde_json::json!({
                "databaseId": id, "status": status, "conclusion": conclusion, "attempt": attempt,
                "url": format!("https://github.com/o/r/actions/runs/{id}"),
            })
            .to_string()
        })
        .collect();
    format!("[{}]", runs.join(","))
}

/// What `gh api <path> -i -H 'If-None-Match: ...'` prints, and exits 1 on, when nothing changed.
const NOT_MODIFIED: &str = "HTTP/2.0 304 Not Modified\nX-Ratelimit-Used: 7\r\n\r\n";

fn put(dir: &Path, file: &str, text: &str) {
    fs::write(dir.join(file), text).unwrap();
}

/// The command lines the stand-in logged for a repository, each as its words.
fn logged(dir: &Path) -> Vec<Vec<String>> {
    fs::read_to_string(dir.join("log"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim_end_matches('\t').split('\t').map(str::to_string).collect())
        .collect()
}

/// The words of the logged lines that are the look at the runs, in the API's form.
fn api_looks(lines: &[Vec<String>]) -> Vec<&Vec<String>> {
    lines
        .iter()
        .filter(|l| l[0] == "api" && l[1].contains("/runs?"))
        .collect()
}

/// The entity tag a logged look sent, if any.
fn sent_tag(look: &[String]) -> Option<&str> {
    look.windows(2)
        .find(|w| w[0] == "-H")
        .and_then(|w| w[1].strip_prefix("If-None-Match: "))
}

fn gh_polling_fast() -> Gh {
    let mut gh = Gh::new(std::env::temp_dir().join("wf-agents-gh-runs-logs"));
    gh.poll = Duration::from_millis(30);
    gh
}

// The wait for a run asks the API of the workflow's runs for the commit with `gh api -i`, names a workflow written as
// the project's configuration writes it (`fast`) by its id, which it finds once in the repository's list and keeps,
// sends the entity tag of the last 200 on each look after it, keeps the runs through a 304 (which gh exits 1 on), and
// gives the run that finished. The reference is GitHub's REST API: conditional requests by `If-None-Match`, and the
// workflow runs of a workflow at `/repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs` with `head_sha`.
#[test]
fn a_wait_asks_the_api_for_the_workflows_runs_and_sends_back_the_tag_of_each_answer() {
    let workflows = serde_json::json!({
        "total_count": 2,
        "workflows": [
            { "id": 11, "name": "full", "path": ".github/workflows/full.yml" },
            { "id": 777, "name": "fast", "path": ".github/workflows/fast.yml" },
        ],
    });
    with_fake_gh(
        "a_wait_asks_the_api_for_the_workflows_runs_and_sends_back_the_tag_of_each_answer",
        |root| {
            let dir = repo_dir(root, "o/r");
            put(&dir, "workflows-1.out", &workflows.to_string());
            put(
                &dir,
                "runs-1.out",
                &api_200(Some("W/\"t1\""), &[(100, "in_progress", None, 1)]),
            );
            put(&dir, "runs-2.out", NOT_MODIFIED);
            put(&dir, "runs-2.err", "gh: HTTP 304\n");
            put(&dir, "runs-2.code", "1");
            put(&dir, "runs-3.out", NOT_MODIFIED);
            put(&dir, "runs-3.err", "gh: HTTP 304\n");
            put(&dir, "runs-3.code", "1");
            put(
                &dir,
                "runs-4.out",
                &api_200(
                    Some("W/\"t2\""),
                    &[
                        (100, "completed", Some("success"), 1),
                        (90, "completed", Some("failure"), 1),
                    ],
                ),
            );
            put(
                &dir,
                "view-1.out",
                r#"{"jobs":[{"name":"lint","conclusion":"success","url":"https://github.com/o/r/actions/runs/100/job/5","databaseId":5}]}"#,
            );
            // Another repository, whose list has two workflows named `ci`, and one with the file name `fast.yml`.
            let dup = repo_dir(root, "o/dup");
            put(
                &dup,
                "workflows-1.out",
                &serde_json::json!({ "total_count": 2, "workflows": [
                    { "id": 1, "name": "ci", "path": ".github/workflows/a.yml" },
                    { "id": 2, "name": "CI", "path": ".github/workflows/b.yml" },
                ]})
                .to_string(),
            );
            let by_file = repo_dir(root, "o/file");
            put(
                &by_file,
                "runs-1.out",
                &api_200(None, &[(5, "completed", Some("success"), 1)]),
            );
            put(&by_file, "view-1.out", r#"{"jobs":[]}"#);
        },
        |root| {
            let gh = gh_polling_fast();
            let checks = gh
                .checks("o/r", "abc123", "fast")
                .expect("the wait ends with the finished run");
            assert_eq!(
                checks
                    .iter()
                    .map(|c| (c.name.as_str(), c.result.as_str(), c.run.as_str()))
                    .collect::<Vec<_>>(),
                [("lint", "green", "https://github.com/o/r/actions/runs/100/job/5")]
            );
            let lines = logged(&root.join("o_r"));
            // The list of workflows is read once, to find `fast` by its name.
            let listings: Vec<&Vec<String>> = lines.iter().filter(|l| l[1].contains("/actions/workflows?")).collect();
            assert_eq!(listings.len(), 1, "{lines:?}");
            assert_eq!(listings[0][1], "repos/o/r/actions/workflows?per_page=100");
            // Four looks, each at the workflow's id, for the commit; the first sends no tag and each after it the tag of
            // the last 200, which a 304 in between leaves as it was.
            let looks = api_looks(&lines);
            assert_eq!(looks.len(), 4, "{lines:?}");
            for look in &looks {
                assert_eq!(
                    look[1],
                    "repos/o/r/actions/workflows/777/runs?head_sha=abc123&per_page=20&exclude_pull_requests=true"
                );
                assert!(look.iter().any(|w| w == "-i"), "{look:?}");
            }
            assert_eq!(
                looks.iter().map(|l| sent_tag(l)).collect::<Vec<_>>(),
                [None, Some("W/\"t1\""), Some("W/\"t1\""), Some("W/\"t1\"")]
            );

            // The workflow's id is remembered: another wait does not read the list again. The stand-in has no fifth
            // answer to a look, so that wait fails, naming what gh said.
            let again = gh.checks("o/r", "abc123", "fast");
            let failure = again.expect_err("the stand-in has no more answers");
            assert_eq!(failure.kind, "network_or_api", "{}", failure.what);
            assert!(
                failure.what.contains("the stand-in has no answer for runs 5"),
                "{}",
                failure.what
            );
            let lines = logged(&root.join("o_r"));
            assert_eq!(lines.iter().filter(|l| l[1].contains("/actions/workflows?")).count(), 1);

            // A name that two workflows have is a failure that names it, and no look is made.
            let failure = gh.checks("o/dup", "abc", "ci").expect_err("two workflows are named ci");
            assert!(failure.what.contains("ci"), "{}", failure.what);
            assert!(api_looks(&logged(&root.join("o_dup"))).is_empty());

            // A file name goes into the path as it is, with no list read.
            gh.checks("o/file", "def", "fast.yml")
                .expect("a file name needs no list");
            let lines = logged(&root.join("o_file"));
            assert_eq!(lines.iter().filter(|l| l[1].contains("/actions/workflows?")).count(), 0);
            assert_eq!(
                api_looks(&lines)[0][1],
                "repos/o/file/actions/workflows/fast.yml/runs?head_sha=def&per_page=20&exclude_pull_requests=true"
            );
        },
    );
}

// A single look (the latest run, the finished checks) is one request that sends no tag, and gives the newest run by its
// number whatever order the API lists them in. The reference is the meaning these had over `gh run list`: the newest
// run of the workflow on the commit, and the checks only of one that finished.
#[test]
fn a_single_look_is_one_request_without_a_tag_and_takes_the_newest_run() {
    with_fake_gh(
        "a_single_look_is_one_request_without_a_tag_and_takes_the_newest_run",
        |root| {
            let dir = repo_dir(root, "o/r");
            put(
                &dir,
                "runs-1.out",
                &api_200(
                    Some("W/\"t\""),
                    &[
                        (8, "completed", Some("success"), 1),
                        (12, "in_progress", None, 1),
                        (9, "completed", Some("failure"), 2),
                    ],
                ),
            );
            put(
                &dir,
                "runs-2.out",
                &api_200(
                    Some("W/\"t\""),
                    &[(12, "in_progress", None, 1), (8, "completed", Some("success"), 1)],
                ),
            );
            put(
                &dir,
                "runs-3.out",
                &api_200(Some("W/\"t\""), &[(12, "completed", Some("success"), 1)]),
            );
            put(
                &dir,
                "view-1.out",
                r#"{"jobs":[{"name":"lint","conclusion":"failure","url":"https://github.com/o/r/actions/runs/12/job/6","databaseId":6}]}"#,
            );
            put(&dir, "view-2.out", "the log\n");
        },
        |root| {
            let gh = gh_polling_fast();
            assert_eq!(gh.latest_run_id("o/r", "abc", "fast.yml").unwrap(), Some(12));
            assert!(
                gh.finished_checks("o/r", "abc", "fast.yml").unwrap().is_none(),
                "the newest run is still going: the finished run before it is not the answer"
            );
            let done = gh
                .finished_checks("o/r", "abc", "fast.yml")
                .unwrap()
                .expect("the run finished");
            assert_eq!(done.len(), 1);
            assert_eq!(done[0].result, "red");
            let lines = logged(&root.join("o_r"));
            let looks = api_looks(&lines);
            assert_eq!(looks.len(), 3, "{lines:?}");
            assert!(looks.iter().all(|l| sent_tag(l).is_none()), "{looks:?}");
        },
    );
}

// A run run again is waited for until the run of the next attempt finishes: the first wait takes the finished run and
// its attempt, `gh run rerun --failed` asks for the new attempt, and a look that still shows the old attempt finished
// is not the answer. The stand-in answers both the listing of `gh run list` and the API's, so that the same test holds
// the meaning of the wait before the engine looked at runs by the API and after it.
#[test]
fn a_run_run_again_is_waited_for_until_its_next_attempt_finishes() {
    with_fake_gh(
        "a_run_run_again_is_waited_for_until_its_next_attempt_finishes",
        |root| {
            let dir = repo_dir(root, "o/r");
            let looks: [Run; 4] = [
                (55, "completed", Some("failure"), 1),
                (55, "completed", Some("failure"), 1),
                (55, "in_progress", None, 2),
                (55, "completed", Some("success"), 2),
            ];
            for (i, run) in looks.iter().enumerate() {
                put(
                    &dir,
                    &format!("runs-{}.out", i + 1),
                    &api_200(Some(&format!("W/\"t{i}\"")), &[*run]),
                );
                put(&dir, &format!("runs-{}.old", i + 1), &run_list(&[*run]));
            }
            // `gh run rerun` prints nothing the engine reads.
            put(&dir, "rerun-1.out", "");
            put(
                &dir,
                "view-1.out",
                r#"{"jobs":[{"name":"test","conclusion":"success","url":"https://github.com/o/r/actions/runs/55/job/9","databaseId":9}]}"#,
            );
        },
        |root| {
            let gh = gh_polling_fast();
            let checks = gh.rerun("o/r", "abc", "fast.yml").expect("the second attempt finished");
            assert_eq!(checks.len(), 1);
            assert_eq!((checks[0].name.as_str(), checks[0].result.as_str()), ("test", "green"));
            let lines = logged(&root.join("o_r"));
            let kinds: Vec<&str> = lines
                .iter()
                .map(|l| {
                    if l[0] == "run" && l[1] == "rerun" {
                        "rerun"
                    } else if l[0] == "run" && l[1] == "view" {
                        "view"
                    } else {
                        "look"
                    }
                })
                .collect();
            // One look finds the failed attempt, the rerun is asked for it, then looks go on until attempt 2 is done.
            assert_eq!(kinds, ["look", "rerun", "look", "look", "look", "view"], "{lines:?}");
            let rerun = &lines[1];
            assert_eq!(&rerun[1..], ["rerun", "55", "-R", "o/r", "--failed"]);
        },
    );
}
