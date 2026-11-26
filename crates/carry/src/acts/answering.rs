//! The host as the act of one round reaches it, which tells the round once the host has answered the act.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use service::{OpenPull, Teller};
use store::Continues;
use tokio::sync::Mutex;

use crate::held::Stop;
use crate::host::{Asking, Branch, Failed, Host, Named, NotMerged, Verdict};

/// The host as the act of one round reaches it. The first of the act's requests that the host answers without an error
/// tells the round that the host answered it ([`Teller::answered`]), as an agent's round tells the first message of the
/// agents' service that is no error: a wait every work shares for the host is over by it while the act is still under
/// way. What is read without asking the host tells nothing, and nor does a verdict or a run of the tests that was
/// stopped.
pub(crate) struct Answering<H> {
    host: Arc<H>,
    teller: Teller,
    told: AtomicBool,
}

impl<H> Answering<H> {
    /// The host as the act of the round `teller` tells of reaches it.
    pub(crate) fn new(host: Arc<H>, teller: Teller) -> Self {
        Answering {
            host,
            teller,
            told: AtomicBool::new(false),
        }
    }

    /// Tells the round that the host answered it, where it did and the round was not told so already.
    fn tell(&self, answered: bool) {
        if answered && !self.told.swap(true, Ordering::Relaxed) {
            self.teller.answered();
        }
    }
}

impl<H: Host> Host for Answering<H> {
    async fn main(&self) -> Result<String, String> {
        let main = self.host.main().await;
        self.tell(main.is_ok());
        main
    }

    async fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        main: &str,
        asking: Asking,
        repository: &Mutex<()>,
        stop: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        let verdict = (self.host)
            .verdict(branch, commit, main, asking, repository, stop)
            .await;
        self.tell(matches!(verdict, Ok(Some(_))));
        verdict
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        let merged = self.host.merge(branch, checked).await;
        self.tell(merged.is_ok());
        merged
    }

    async fn continued(&self, what: &Continues) -> Result<String, Failed> {
        let continued = self.host.continued(what).await;
        self.tell(continued.is_ok());
        continued
    }

    fn kept_main(&self) -> String {
        self.host.kept_main()
    }

    async fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        asking: Asking,
        stop: &mut Stop,
    ) -> Result<Option<String>, Failed> {
        let ran = self.host.run_tests(branch, named, asking, stop).await;
        self.tell(matches!(ran, Ok(Some(_))));
        ran
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        let ended = self.host.end(branch, merged).await;
        self.tell(ended.is_ok());
        ended
    }

    async fn reopen(&self, branch: &Branch<'_>) -> Result<(), String> {
        let reopened = self.host.reopen(branch).await;
        self.tell(reopened.is_ok());
        reopened
    }

    fn pull_at(&self, number: u64) -> Option<String> {
        self.host.pull_at(number)
    }

    async fn open_pulls(&self) -> Result<Option<Vec<OpenPull>>, String> {
        let open = self.host.open_pulls().await;
        self.tell(open.is_ok());
        open
    }
}
