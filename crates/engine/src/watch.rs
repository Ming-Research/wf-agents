//! Watching an agent's round while it runs: what its command line's stream tells goes to the owner's page and the run
//! guard ([`sessions::guard`]), and answers whether the run goes on. Reading the stream must never wait, or the command
//! line's pipe fills: the judgement is arithmetic, and what is kept is handed to a thread of the round's own, with a
//! connection of its own, since the step's is held mutably by nobody while it waits for the run. Nothing the ledger or
//! the lines fail at fails the round: the guard then says so and sees less.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use db::Actor;
use flow::format::{GUARDED, Rounds};
use sessions::guard::{self, Action, Crossing, Lines, Run};

use crate::agent::{Call, Next, Seen};
use crate::work::{Round, RoundId};
use crate::{Activity, Engine, Measured, database, failed, handover};

/// What a round starting is judged from.
pub struct Start<'a> {
    pub graph: &'a str,
    pub cell: &'a str,
    pub section: &'a Rounds,
    /// The cell's rounds, newest first.
    pub at_cell: &'a [Round],
    /// When the owner last let the work go on.
    pub resumed: Option<i64>,
    pub model: &'a str,
    pub message: &'a str,
    /// The rounds of the session it runs in, oldest first; none for a new session.
    pub session: &'a [Round],
}

impl Start<'_> {
    /// What comes of a signal that stops a round: it stops it, but in a flow version with no row for that, or for the
    /// round the owner let go on after its cell's last round was stopped, where it is only recorded.
    fn past(&self) -> Action {
        let stops = self.section.guard.is_some() && self.section.failed.kinds.iter().any(|k| k.name == GUARDED);
        if !stops {
            Action::Noted
        } else if failed::waived(self.at_cell, self.resumed) {
            Action::Waived
        } else {
            Action::Stopped
        }
    }

    /// The lines the round is judged by, computed once, from the rounds kept; none in a flow version with no rule for
    /// them, or when the rounds cannot be read.
    fn lines(&self, e: &Engine) -> Lines {
        let Some(rule) = &self.section.guard else {
            return Lines::default();
        };
        match guard::samples(e.db.conn(), self.graph) {
            Ok(all) => Lines::of(&all, self.graph, self.cell, self.model, rule.samples as usize),
            Err(x) => {
                e.say(&format!("cell {}: the run guard has no lines: {x}", self.cell));
                Lines::default()
            }
        }
    }

    /// The round as the guard starts it.
    pub fn run(&self, e: &Engine) -> Run {
        let before = guard::last_prompt(e.db.conn(), self.session).unwrap_or_else(|x| {
            e.say(&format!("cell {}: the run guard has no last prompt: {x}", self.cell));
            None
        });
        let given = guard::given(self.message, self.session);
        Run::new(self.lines(e), self.past(), self.model, given, before)
    }
}

/// What the round's own thread keeps.
enum Note {
    Call(Call),
    Crossed(Crossing),
}

/// A round being watched.
pub struct Watch {
    round: RoundId,
    run: RefCell<Run>,
    /// Its session as the hand-over rule sees it, for a round the engine may ask to hand over.
    hand: RefCell<Option<handover::Watched>>,
    notes: Option<Sender<(Note, i64)>>,
    writer: Option<JoinHandle<()>>,
    activity: Arc<Activity>,
    measure: Arc<Measured>,
}

impl Watch {
    /// Starts watching a round, with a thread that keeps what it does; `hand` is its session as the hand-over rule sees
    /// it, for a round the engine may ask to hand over.
    pub fn begin(e: &Engine, round: RoundId, run: Run, hand: Option<handover::Watched>) -> Watch {
        let hooks = &e.shared.hooks;
        let (notes, taken) = mpsc::channel();
        let (data, say, commit) = (e.data.clone(), Arc::clone(&hooks.say), hooks.commit.clone());
        let writer = std::thread::spawn(move || keep(&data, round, &taken, &*say, commit));
        Watch {
            round,
            run: RefCell::new(run),
            hand: RefCell::new(hand),
            notes: Some(notes),
            writer: Some(writer),
            activity: Arc::clone(&hooks.activity),
            measure: Arc::clone(&hooks.measure),
        }
    }

    fn note(&self, note: Note, at: i64) {
        if let Some(n) = &self.notes {
            // A thread that ended has said why.
            let _ = n.send((note, at));
        }
    }

    /// Takes what the run does; whether it goes on.
    pub fn see(&self, seen: Seen) -> Next {
        let RoundId { work, n } = self.round;
        if let Seen::Doing(what) = &seen {
            (self.activity)(work, n, what.as_str());
            return Next::Go;
        }
        if let Some(h) = self.hand.borrow_mut().as_mut() {
            h.see(&seen);
        }
        let (past, live) = {
            let mut run = self.run.borrow_mut();
            let past = run.see(&seen);
            (past, run.live())
        };
        let at = db::now();
        if let Seen::Call(c) = seen {
            self.note(Note::Call(c), at);
            (self.measure)(work, n, live);
        }
        let mut next = Next::Go;
        for c in past {
            if c.action == Action::Stopped && next == Next::Go {
                next = Next::Stop(c.words());
            }
            self.note(Note::Crossed(c), at);
        }
        next
    }

    /// Stops watching, once all it noted is kept; what the round came to, to the guard and to the hand-over rule.
    pub fn end(mut self) -> (Run, Option<handover::Watched>) {
        drop(self.notes.take());
        if let Some(t) = self.writer.take() {
            let _ = t.join();
        }
        (self.run.into_inner(), self.hand.into_inner())
    }
}

/// Keeps what a round did as it comes, on a connection of its own, its writes told as every other write is.
fn keep(
    data: &Path,
    round: RoundId,
    notes: &Receiver<(Note, i64)>,
    say: &(dyn Fn(&str) + Send + Sync),
    commit: Option<db::Listener>,
) {
    let mut db = match database::open(data) {
        Ok(db) => db,
        Err(x) => return say(&format!("{round}: nothing it does is kept: {x}")),
    };
    if let Some(f) = commit
        && let Err(x) = db.on_commit(f)
    {
        say(&format!("{round}: what it does is kept untold: {x}"));
    }
    for (note, at) in notes {
        let kept = db.write(Actor::Engine, |tx| {
            match &note {
                Note::Call(c) => guard::keep_call(tx, round, c, at)?,
                Note::Crossed(c) => guard::keep_crossing(tx, round, c, at)?,
            }
            Ok(())
        });
        if let Err(x) = kept {
            say(&format!("{round}: what it did could not be kept: {x}"));
        }
    }
}
