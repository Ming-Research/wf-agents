//! The most a work may spend: the installation's budget of a work, with what each letting-on of it allows beyond that.
//!
//! A work whose rounds, keep-warm pings and forks have spent more than it may waits for the owner as its round ends (crate
//! `carry`). Held to its budget alone, such a work ended the round after each letting-on over the budget too and was
//! paused again, so a letting-on bought one round and no more; so each letting-on of a work the engine paused for its
//! budget allows it [`Budget::let_on`] beyond what it had spent by then. Counted beyond the bar it was paused at
//! instead, a round that spent more than the allowance left the work over its new bar the moment it was let on, and it
//! was paused again at the end of its next round.
//!
//! The bar a work is held to is worked out from its records, nothing of it being stored (principle
//! `state-from-records`): the pauses in order, a pause of the engine's where the work had spent more than it then
//! might being the one for the budget. That tells it from a pause of the owner's, and from a pause of a round that
//! failed while the work was under its bar, neither of which allows anything more.

use decide::{Pauser, RunId, Seq};
use work::Work;

/// The most a work may spend, as the installation sets it, in millionths of a dollar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The budget of a work, `agents.work_budget_usd`.
    pub work: u64,
    /// What each letting-on of a work the engine paused for its budget allows it beyond its budget,
    /// `agents.let_on_allowance_usd`.
    pub let_on: u64,
}

impl Budget {
    /// The most the work may spend now: its budget, until the owner lets it on after the engine paused it for
    /// spending more than it may; from then, what it had spent by that letting-on and the allowance more.
    pub fn bar(&self, work: &Work) -> u64 {
        let mut bar = self.work;
        // Whether the pause that holds the work now is the engine's for the budget: the letting-on that lifts such a
        // pause, and only such a one, allows the allowance more.
        let mut for_the_budget = false;
        for pause in &work.records.pauses {
            match pause.paused {
                true => for_the_budget = pause.by == Pauser::Engine && spent_by(work, pause.at) > bar,
                false => {
                    if for_the_budget {
                        bar = spent_by(work, pause.at).saturating_add(self.let_on);
                    }
                    for_the_budget = false;
                }
            }
        }
        bar
    }
}

/// What the work had spent by this place, in millionths of a dollar: every round of it that had begun by then, with
/// every keep-warm ping and fork of it that had ended, which count in its spend as its rounds do. The rounds are counted from
/// where each began, not from where it ended: the engine records the pause of a work before the return of the round
/// whose end pauses it (crate `service`), so the round that took the work over what it might spend ends after that
/// pause, and counted from its end it would be left out of what the pause was for.
fn spent_by(work: &Work, at: Seq) -> u64 {
    let r = &work.records;
    let mut counted: Vec<RunId> = Vec::new();
    let mut usd = 0;
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        if round.at < at {
            usd += work.spent.get(&(round.run, k)).map_or(0, |spent| spent.usd_micros);
        }
    }
    let beside = (work.spent_beside_rounds().into_iter()).filter(|call| call.at <= at);
    usd + beside.map(|call| call.spent.usd_micros).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use decide::{Pause, Returned, Round};
    use work::{Forked, Pinged, Spent};

    use super::*;

    /// A work whose rounds, of one run, ended at these places having spent this much in millionths of a dollar, and
    /// which was paused or let on at these places: by whom, and whether it was a pause.
    fn work(rounds: &[(u64, u64)], pauses: &[(u64, Pauser, bool)]) -> Work {
        let mut work = Work::default();
        let run = RunId(1);
        for (number, (at, usd)) in rounds.iter().enumerate() {
            work.records.rounds.push(Round { run, at: Seq(at - 1) });
            work.records.returned.push(Returned { run, at: Seq(*at) });
            let spent = Spent {
                usd_micros: *usd,
                ..Spent::default()
            };
            work.spent.insert((run, number + 1), spent);
        }
        for (at, by, paused) in pauses {
            work.records.pauses.push(Pause {
                paused: *paused,
                at: Seq(*at),
                by: *by,
                why: None,
                waits_for: Vec::new(),
            });
        }
        work
    }

    /// A budget of a dollar, where each letting-on of a work the engine paused for it allows fifty cents more.
    const BUDGET: Budget = Budget {
        work: 1_000_000,
        let_on: 500_000,
    };

    #[test]
    fn a_work_with_no_pause_of_the_engines_for_its_budget_is_held_to_its_budget() {
        let rounds = [(10, 600_000), (20, 600_000)];
        assert_eq!(BUDGET.bar(&work(&rounds, &[])), 1_000_000, "never paused");
        // A work added paused by the owner, let on with nothing spent yet, and one the owner paused and let on again
        // over its budget: neither letting-on is after a pause of the engine's.
        let owners = [(1, Pauser::Owner, true), (2, Pauser::Owner, false)];
        assert_eq!(BUDGET.bar(&work(&rounds, &owners)), 1_000_000);
        let later = [(25, Pauser::Owner, true), (26, Pauser::Owner, false)];
        assert_eq!(BUDGET.bar(&work(&rounds, &later)), 1_000_000);
        // A pause of the engine's where the work had spent no more than it might, as of a round that failed: the
        // letting-on after it allows nothing more.
        let failed = [(15, Pauser::Engine, true), (16, Pauser::Owner, false)];
        assert_eq!(BUDGET.bar(&work(&rounds, &failed)), 1_000_000, "$0.60 of its dollar");
    }

    #[test]
    fn each_letting_on_after_a_pause_of_the_engines_for_the_budget_allows_the_allowance_beyond_what_was_spent() {
        // Two rounds of sixty cents, then two of thirty, with the engine pausing the work as one took it over what it
        // then might spend and the owner letting it on after.
        let rounds = [(10, 600_000), (20, 600_000), (30, 300_000), (40, 300_000)];
        let once = [(21, Pauser::Engine, true), (22, Pauser::Owner, false)];
        assert_eq!(
            BUDGET.bar(&work(&rounds, &once)),
            1_700_000,
            "$1.20 spent, and fifty cents more"
        );
        // Its third round, at thirty cents, leaves it under the $1.70 it may now spend, and its fourth takes it over.
        let under = [once[0], once[1], (31, Pauser::Engine, true), (32, Pauser::Owner, false)];
        assert_eq!(
            BUDGET.bar(&work(&rounds, &under)),
            1_700_000,
            "$1.50 of its $1.70: no pause for the budget"
        );
        let twice = [once[0], once[1], (41, Pauser::Engine, true), (42, Pauser::Owner, false)];
        assert_eq!(
            BUDGET.bar(&work(&rounds, &twice)),
            2_300_000,
            "$1.80 of its $1.70, and fifty cents more"
        );
        // Its pause holding, the bar is the one the pause was judged by: the letting-on that lifts it raises it.
        let holding = [once[0], once[1], (41, Pauser::Engine, true)];
        assert_eq!(BUDGET.bar(&work(&rounds, &holding)), 1_700_000);
    }

    #[test]
    fn a_letting_on_after_a_round_that_spent_more_than_the_allowance_leaves_the_work_room_to_go_on() {
        // A round of a dollar and a half takes the work from sixty cents to $2.10, past its dollar by more than the
        // fifty cents a letting-on allows. Counted from the bar it was paused at, the work would still be over its
        // new bar as it was let on, and paused again at the end of its next round.
        let rounds = [(10, 600_000), (20, 1_500_000)];
        let paused = [(21, Pauser::Engine, true), (22, Pauser::Owner, false)];
        let bar = BUDGET.bar(&work(&rounds, &paused));
        assert_eq!(bar, 2_600_000, "$2.10 spent, and fifty cents more");
        assert!(bar > 2_100_000, "room to go on");
    }

    #[test]
    fn the_round_whose_end_pauses_the_work_counts_in_what_it_had_spent_at_that_pause() {
        // The engine records the pause of a work before the return of the round whose end pauses it, so that round's
        // spend is recorded after the pause: counted from where the round began, it is what the pause was for.
        let mut work = Work::default();
        let run = RunId(1);
        let pause = |paused, at, by| Pause {
            paused,
            at: Seq(at),
            by,
            why: None,
            waits_for: Vec::new(),
        };
        work.records.rounds.push(Round { run, at: Seq(1) });
        work.records.pauses.push(pause(true, 2, Pauser::Engine));
        work.records.returned.push(Returned { run, at: Seq(3) });
        work.spent.insert(
            (run, 1),
            Spent {
                usd_micros: 1_200_000,
                ..Spent::default()
            },
        );
        assert_eq!(BUDGET.bar(&work), 1_000_000, "its pause holding");
        work.records.pauses.push(pause(false, 4, Pauser::Owner));
        assert_eq!(BUDGET.bar(&work), 1_700_000, "$1.20 spent, and fifty cents more");
    }

    #[test]
    fn a_keep_warm_ping_counts_in_what_the_work_had_spent_at_a_pause() {
        // One round of ninety cents and a ping of twenty: the engine paused the work as the ping ended, over its
        // dollar, and the letting-on after allows fifty cents beyond the $1.10 spent.
        let mut work = work(
            &[(10, 900_000)],
            &[(21, Pauser::Engine, true), (22, Pauser::Owner, false)],
        );
        work.pinged.push(Pinged {
            at: Seq(20),
            line: "engineer".to_string(),
            session: 0,
            spent: Spent {
                usd_micros: 200_000,
                ..Spent::default()
            },
            failed: None,
        });
        assert_eq!(BUDGET.bar(&work), 1_600_000);
    }

    #[test]
    fn a_fork_counts_in_what_the_work_had_spent_at_a_pause_as_a_ping_does() {
        // The same work as with the ping, a fork of twenty cents at the place the ping was at: the same bar, and none
        // for a fork that ended after the pause.
        let mut work = work(
            &[(10, 900_000)],
            &[(21, Pauser::Engine, true), (22, Pauser::Owner, false)],
        );
        let fork = |at, usd_micros| Forked {
            at: Seq(at),
            run: RunId(1),
            round: 1,
            usd_micros,
            uncached_tokens: 0,
            cache_written_tokens: None,
            cached_tokens: 0,
            output_tokens: 0,
        };
        work.forked.push(fork(20, 200_000));
        assert_eq!(BUDGET.bar(&work), 1_600_000);
        work.forked.push(fork(30, 700_000));
        assert_eq!(
            BUDGET.bar(&work),
            1_600_000,
            "a fork after the letting-on is not in what had been spent"
        );
    }
}
