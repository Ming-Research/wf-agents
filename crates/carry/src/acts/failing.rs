//! The kind of cause of an act of the engine's that failed. A failure git, ssh, `gh` or the host give where the network
//! fails is the network's, and is waited out with every work where a look at the act's own host finds it out of reach,
//! or else tried again as a failure in passing; any other pauses the work, as one the act cannot get past. An act of a
//! host no network reaches, as the local repository, fails for no network. An act that knows its cause says it itself
//! ([`Failed::passing`]).

use decide::run::Failure;

/// Why an act could not be done, and the kind of its cause where the act knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Failed {
    pub(crate) why: String,
    pub(crate) kind: Option<Failure>,
}

impl Failed {
    /// An act that failed for a cause that passes by itself: it is tried again after a wait.
    pub(crate) fn passing(why: String) -> Self {
        Failed {
            why,
            kind: Some(Failure::Passing),
        }
    }
}

impl From<String> for Failed {
    fn from(why: String) -> Self {
        Failed { why, kind: None }
    }
}

/// The kind of cause of a failed act: the one the act said, or by what it failed with, the act's host being looked at
/// only where its words are the network's. `reached` looks, and gives none where the host is reached by no network.
pub(crate) async fn kind(failed: &Failed, reached: impl AsyncFnOnce() -> Option<bool>) -> Failure {
    if let Some(kind) = failed.kind {
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
        // What the act said of its cause stands.
        let moved = Failed::passing("main has moved".to_string());
        assert_eq!(kind(&moved, async || panic!("looked")).await, Failure::Passing);
    }
}
