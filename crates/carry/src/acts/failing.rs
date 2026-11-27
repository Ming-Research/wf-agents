//! The kind of cause of an act of the engine's that failed. Where the host's answer tells it, or what the act found,
//! the act says it itself ([`Failed`]): a fault of the host's own or its limit on requests, and CI that never started,
//! only keep the act from running for now, and are waited out and tried again (a round of the act counts them toward no
//! pause); a refusal, a person's doing on what the work holds there and a fault of the configuration pause the work, with
//! why, for the owner. Where nothing tells it, as where a command gave no answer, a failure git, ssh, `gh` or the host
//! give where the network fails is the network's, and is waited out with every work where a look at the act's own host
//! finds it out of reach, or else tried again as a failure in passing; any other pauses the work, as one the act cannot
//! get past. A failure said to be the network's by the act itself is looked at the same way. An act of a host no network
//! reaches, as the local repository, fails for no network.

use decide::Obstacle;
use decide::run::Failure;

use crate::host::Failed;

/// A failure of what a git command or `gh` gave no kind for: where its words are the network's, it only keeps the host
/// from doing what it was asked for now, and what asked is held and asked again after a wait; any other is as it is.
pub(crate) fn told_by_its_words(why: String) -> Failed {
    told(Failed::from(why))
}

/// A failure as [`told_by_its_words`] reads it, where it has no kind and is not held already: a failure that says what
/// it is is left as it is.
pub(crate) fn told(failed: Failed) -> Failed {
    if failed.kind.is_none() && network(&failed.why) {
        return Failed::held(failed.why, Obstacle::Network, None);
    }
    failed
}

/// The kind of cause of a failed act: the one the act said, or by the words it failed with, the act's host being looked
/// at only where they are the network's, or where the act said the host was kept from it by the network (held by
/// [`Obstacle::Network`]): where the look finds the host out of reach, the cause is shared by every work, and is not
/// one of this act's. `reached` looks, and gives none where the host is reached by no network.
pub(crate) async fn kind(failed: &Failed, reached: impl AsyncFnOnce() -> Option<bool>) -> Failure {
    if let Some(kind) = failed.kind {
        if let Some(hindrance) = failed.hindrance
            && hindrance.obstacle == Obstacle::Network
            && reached().await == Some(false)
        {
            return Failure::Offline;
        }
        return kind;
    }
    if !network(&failed.why) {
        return Failure::Other;
    }
    match reached().await {
        Some(true) => Failure::Passing,
        Some(false) => Failure::Offline,
        None => Failure::Other,
    }
}

/// Whether what an act failed with is a failure of the network that passes by itself, as git, ssh, `gh` and a host
/// word it: a connection refused, reset or closed, a host that does not resolve, a timeout, a server that fails. Words
/// it does not know are not the network's, so that an act that fails for any other cause (the configuration, a
/// permission, a conflict) is not run again.
fn network(why: &str) -> bool {
    const SIGNS: &[&str] = &[
        "open failed",
        "administratively prohibited",
        "stdio forwarding failed",
        "could not resolve host",
        "name or service not known",
        "temporary failure in name resolution",
        "nodename nor servname provided",
        "connection reset",
        "connection refused",
        "connection closed",
        "connection abort",
        "failed to connect",
        "couldn't connect to server",
        "timed out",
        "network is unreachable",
        "network is down",
        "no route to host",
        "broken pipe",
        "remote end hung up",
        "unexpected disconnect",
        "early eof",
        "rpc failed",
        "returned error: 5",
        "exchange_identification",
        "http 502",
        "http 503",
        "http 504",
        // As GitHub's command line says it, and the network library it is built on.
        "error connecting to",
        "check your internet connection",
        "i/o timeout",
        "no such host",
        "tls handshake timeout",
        // A command of git or `gh` the engine stopped for taking too long, as it stalled on the network.
        "did not end within",
    ];
    let why = why.to_lowercase();
    SIGNS.iter().any(|sign| why.contains(sign))
}

#[cfg(test)]
mod tests {
    use decide::Hindrance;

    use super::*;

    // A connection to the host that fails is the network's; a missing permission, an unknown branch, a fault of the
    // configuration and words it does not know are not, so that an act that fails for those is not run again.
    #[test]
    fn an_act_is_run_again_only_for_a_failure_the_network_gives() {
        assert!(network(
            "channel 0: open failed: administratively prohibited: open failed\nstdio forwarding failed\n\
             ssh_exchange_identification: Connection closed by remote host\nfatal: Could not read from remote repository."
        ));
        assert!(network(
            "ssh: Could not resolve hostname github.com: Name or service not known"
        ));
        assert!(network(
            "fatal: unable to access 'https://github.com/o/r/': Failed to connect to github.com port 443: Connection \
             timed out"
        ));
        assert!(network(
            "error: RPC failed; HTTP 502 curl 22 The requested URL returned error: 502"
        ));
        assert!(network("gh: HTTP 503: Service Unavailable"));
        assert!(network(
            "error connecting to api.github.com\ncheck your internet connection or https://githubstatus.com"
        ));
        assert!(network(
            "Post \"https://api.github.com/graphql\": dial tcp: lookup api.github.com: no such host"
        ));
        assert!(network(
            "gh api GET repos/o/r/pulls did not end within 120 seconds, and was stopped"
        ));
        assert!(!network(
            "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository."
        ));
        assert!(!network("fatal: couldn't find remote ref main"));
        assert!(!network(
            "GitHub did not merge the pull request: HTTP 405: At least 1 approving review is required"
        ));
        assert!(!network(""));
    }

    // A call whose command gave no kind and whose words are the network's only keeps the host from doing what it was
    // asked for now: it is held, and asked again. Any other words are as they were, with no kind and nothing held.
    #[test]
    fn a_failure_with_no_kind_is_held_only_where_its_words_are_the_networks() {
        let gone = told_by_its_words("fatal: unable to access: Connection timed out".to_string());
        assert_eq!(
            gone.hindrance,
            Some(Hindrance {
                obstacle: Obstacle::Network,
                retry: None
            })
        );
        assert_eq!(gone.kind, Some(Failure::Held));
        let refused = told_by_its_words("fatal: couldn't find remote ref main".to_string());
        assert_eq!((refused.hindrance, refused.kind), (None, None));
    }

    // A failure that says what it is, a fault of GitHub's own or a refusal, is left as it is though its words name what the
    // network's do.
    #[test]
    fn a_failure_that_says_what_it_is_is_left_as_it_is_whatever_its_words() {
        let fault = told(Failed::held("HTTP 502: Bad Gateway", Obstacle::Fault, None));
        assert_eq!(fault.hindrance.map(|held| held.obstacle), Some(Obstacle::Fault));
        let refused = told(Failed::refused("HTTP 422: the request timed out"));
        assert_eq!((refused.hindrance, refused.kind), (None, Some(Failure::Other)));
    }

    // A round of an act that GitHub's limit, a fault of its own or no runner keeps from running is held at once, with
    // no look at the host; one the network keeps from running is held where the look reaches the host or looks at
    // none, and waits with every work where the look finds the host out of reach.
    #[tokio::test]
    async fn a_failure_that_keeps_the_host_from_an_act_for_now_is_held_and_one_of_the_network_is_looked_at() {
        let limit = Hindrance {
            obstacle: Obstacle::Limit,
            retry: Some(decide::Retry::In(3600)),
        };
        let limited = Failed::held("HTTP 429: rate limit", limit.obstacle, limit.retry);
        assert_eq!(kind(&limited, async || panic!("looked")).await, Failure::Held);
        for obstacle in [Obstacle::Fault, Obstacle::NoRunner] {
            let held = Failed::held("kept", obstacle, None);
            assert_eq!(kind(&held, async || panic!("looked")).await, Failure::Held);
        }
        let down = Failed::held("Connection timed out", Obstacle::Network, None);
        assert_eq!(kind(&down, async || Some(false)).await, Failure::Offline);
        assert_eq!(kind(&down, async || Some(true)).await, Failure::Held);
        assert_eq!(kind(&down, async || None).await, Failure::Held);
    }

    #[tokio::test]
    async fn a_failure_of_the_network_waits_with_every_work_where_the_look_at_the_host_finds_it_gone() {
        let timed_out = Failed::from("fatal: unable to access: Connection timed out".to_string());
        assert_eq!(kind(&timed_out, async || Some(false)).await, Failure::Offline);
        assert_eq!(kind(&timed_out, async || Some(true)).await, Failure::Passing);
        // A host no network reaches fails for no network.
        assert_eq!(kind(&timed_out, async || None).await, Failure::Other);
        // Words that are not the network's are not looked at.
        let refused = Failed::from("the pull request goes into next, not into main".to_string());
        assert_eq!(kind(&refused, async || panic!("looked")).await, Failure::Other);
        // What the act said of its cause stands: a refusal whose words name a timeout is no failure of the network.
        let moved = Failed::passing("main has moved".to_string());
        assert_eq!(kind(&moved, async || panic!("looked")).await, Failure::Passing);
        let refused = Failed::refused("GitHub did not merge the pull request: HTTP 422: the request timed out");
        assert_eq!(kind(&refused, async || panic!("looked")).await, Failure::Other);
    }
}
