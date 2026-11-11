//! The copies kept as assets of a release with `gh`: what it is asked, with what limit, and that no call can hang.

use super::*;

/// The limit of an upload is the runner's limit and the time the file takes at the lowest rate.
#[test]
fn the_limit_of_an_upload_is_the_runners_and_the_time_the_file_takes_at_the_lowest_rate() {
    let floor = Duration::from_secs(120);
    assert_eq!(GhOffsite::upload_limit(floor, 0), floor);
    assert_eq!(
        GhOffsite::upload_limit(floor, 100 * UPLOAD_RATE),
        Duration::from_secs(220)
    );
    // A copy of 200 MB is bounded under an hour at the rate, so that a call that hangs is ended.
    let big = GhOffsite::upload_limit(floor, 200 * 1_000_000);
    assert!(big < Duration::from_secs(3600), "{big:?}");
    assert!(big > Duration::from_secs(3000), "{big:?}");
}

#[cfg(unix)]
mod stand_in {
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    /// A `gh` that keeps one release in a directory: it is not there until made, it answers what GitHub does of the
    /// repository, it logs each call and the token it was given, and it fails every call while the file `down` is there
    /// and hangs on every call while the file `hang` is.
    const GH: &str = r#"#!/bin/sh
d='DIR'
echo "$*" >> "$d/calls"
echo "$GH_TOKEN" > "$d/token"
if [ -f "$d/down" ]; then echo "network is down" >&2; exit 1; fi
if [ -f "$d/hang" ]; then sleep 30; fi
case "$1 $2" in
"api --include")
  if [ -f "$d/public" ]; then body='{"private":false}'; else body='{"private":true}'; fi
  printf 'HTTP/2.0 200 OK\r\n\r\n%s' "$body" ;;
"release view")
  if [ ! -f "$d/release" ]; then echo "release not found" >&2; exit 1; fi
  case "$*" in *assets*) cat "$d/assets" ;; esac ;;
"release create") touch "$d/release"; : > "$d/assets" ;;
"release upload") basename "$4" >> "$d/assets" ;;
"release delete-asset") grep -v -x "$4" "$d/assets" > "$d/assets.new"; mv "$d/assets.new" "$d/assets" ;;
*) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#;

    fn place() -> (TempDir, GhOffsite) {
        let dir = tempfile::tempdir().expect("a directory");
        let script = dir.path().join("gh");
        std::fs::write(&script, GH.replace("DIR", &dir.path().display().to_string())).expect("written");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
        let config = dir.path().join("config");
        let place = GhOffsite::new(&script.display().to_string(), "owner/name", "the-token", &config);
        (dir, place)
    }

    fn calls(dir: &TempDir) -> Vec<String> {
        std::fs::read_to_string(dir.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    // The first upload to a place with no release made yet works, and a listing that fails for a real reason is an
    // error, never an empty place.
    #[tokio::test]
    async fn the_first_upload_to_a_place_with_no_release_works_and_a_failing_listing_is_no_empty_place() {
        let (dir, place) = place();
        std::fs::write(dir.path().join("down"), "").expect("written");
        assert!(
            place.list().await.is_err(),
            "a place that cannot be reached is no empty place"
        );
        std::fs::remove_file(dir.path().join("down")).expect("removed");
        assert_eq!(place.list().await, Ok(vec![]), "no release is an empty place");
        let copy = dir.path().join("store-0000000000001.sqlite.gz");
        std::fs::write(&copy, "copy").expect("written");
        place.upload(&copy).await.expect("the first upload");
        assert_eq!(
            place.list().await,
            Ok(vec!["store-0000000000001.sqlite.gz".to_string()])
        );
        place.remove("store-0000000000001.sqlite.gz").await.expect("removed");
        assert_eq!(place.list().await, Ok(vec![]));
    }

    // What `gh` is asked names the release, the repository and the file, and the token is in its environment alone.
    #[tokio::test]
    async fn what_gh_is_asked_names_the_release_the_repository_and_the_file_and_the_token_is_not_among_it() {
        let (dir, place) = place();
        let copy = dir.path().join("store-0000000000002.sqlite.gz");
        std::fs::write(&copy, "copy").expect("written");
        assert_eq!(place.private().await, Ok(true));
        place.upload(&copy).await.expect("uploaded");
        place.list().await.expect("listed");
        place.remove("store-0000000000002.sqlite.gz").await.expect("removed");
        let calls = calls(&dir);
        let upload = format!(
            "release upload database-backups {} --repo owner/name --clobber",
            copy.display()
        );
        assert!(calls.contains(&upload), "{calls:?}");
        assert!(
            calls.contains(
                &"release view database-backups --repo owner/name --json assets --jq .assets[].name".to_string()
            ),
            "{calls:?}"
        );
        assert!(
            calls.contains(
                &"release delete-asset database-backups store-0000000000002.sqlite.gz --repo owner/name --yes"
                    .to_string()
            ),
            "{calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|c| c.starts_with("api --include") && c.contains("repos/owner/name")),
            "{calls:?}"
        );
        assert!(
            calls.iter().all(|c| !c.contains("the-token")),
            "the token is in no argument: {calls:?}"
        );
        let token = std::fs::read_to_string(dir.path().join("token")).expect("the token was given");
        assert_eq!(token.trim(), "the-token");
    }

    // A repository made public is said so, and each call asks.
    #[tokio::test]
    async fn a_repository_that_is_not_private_is_said_not_to_be() {
        let (dir, place) = place();
        assert_eq!(place.private().await, Ok(true));
        std::fs::write(dir.path().join("public"), "").expect("written");
        assert_eq!(place.private().await, Ok(false));
    }

    // Every call ends: one that never does is stopped within its limit and is an error, so it holds nothing.
    #[tokio::test]
    async fn a_call_that_never_ends_is_stopped_within_its_limit_and_is_an_error() {
        let (dir, place) = place();
        let place = place.within(Duration::from_millis(300));
        std::fs::write(dir.path().join("hang"), "").expect("written");
        let copy = dir.path().join("store-0000000000003.sqlite.gz");
        std::fs::write(&copy, "copy").expect("written");
        let began = std::time::Instant::now();
        for stopped in [
            place.upload(&copy).await.expect_err("an upload that hangs is stopped"),
            place.list().await.expect_err("a listing that hangs is stopped"),
            place.remove("x").await.expect_err("a removal that hangs is stopped"),
            place.private().await.expect_err("a question that hangs is stopped"),
        ] {
            assert!(stopped.contains("did not end"), "{stopped}");
        }
        assert!(began.elapsed() < Duration::from_secs(10), "{:?}", began.elapsed());
    }
}
