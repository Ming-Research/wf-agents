//! Handing a line's session over to a new one once keeping it has cost as much as starting anew would
//! ([`sessions::handover`]). The engine asks the agent to hand over its progress after its next tool use
//! ([`crate::hook`]), or at the top of its next round at the same cell; at another cell the session manager starts the
//! new session itself, since a round there has no progress to hand over. The agent hands in the form `handover`, and the
//! hand-over cell's step ([`hand_over`]) ends the session, opens the new one its cell carries on in, puts back what was
//! said to the round, and passes the hand-over on. Each round the engine may ask records, once its session came due,
//! where and what it came to ([`Watched::record`]).

use std::collections::BTreeSet;

use db::Actor;
use serde_json::{Value, json};
use sessions::cli::Seen;
use sessions::handover::{Judged, Rent};

use crate::inputs::Gathered;
use crate::work::{self, Work};
use crate::{Engine, agent, failed};

/// The form an agent hands its progress over in, and the row it takes to the hand-over cell.
pub const FORM: &str = "handover";

/// Whether an agent's cell may hand its progress over: it has the form, as every agent's cell of a flow version with
/// the hand-over has.
pub fn takes(cell: &flow::CompiledCell) -> bool {
    cell.forms.iter().any(|f| f.field.as_deref() == Some(FORM))
}

/// What asks the agent to hand its progress over, after a tool use or at the top of a round.
pub fn ask() -> String {
    "Keeping this session now costs as much as starting a new one would: hand in handover, now or once the step in \
     hand is done. Write in it what you did, where it is, what is left in order, and what the next session must know \
     that the worktree does not show; a new session carries on from it at this cell, given the cell's whole input."
        .to_string()
}

/// A running round's session as the hand-over rule sees it, call by call: the rent its session had paid before the
/// round, and the round's own calls as the stream tells them; once due, the call it came due at and the tool uses
/// after. Kept for a round the engine may ask to hand over.
#[derive(Debug, Clone)]
pub struct Watched {
    rent: Rent,
    model: String,
    /// The calls told so far, each once, subagents' included: the ledger numbers them in this order.
    told: BTreeSet<String>,
    /// The call it came due at, by its number in the ledger, 0 for due before the round's first call, with the rent and
    /// the buy then.
    due: Option<(i64, f64, f64)>,
    /// The tool uses the stream told after it came due, after each of which the hook asked.
    tools_after: i64,
    /// Whether the round's message asked it to hand over.
    at_top: bool,
}

impl Watched {
    /// A round's watch from what its session had paid before it (`rent`, nothing for a new session), on `model` where a
    /// call names none; `at_top` when its message asks it to hand over.
    pub fn new(rent: Rent, model: &str, at_top: bool) -> Watched {
        let due = match rent.judged() {
            Some(Judged::Due { rent, buy }) => Some((0, rent, buy)),
            _ => None,
        };
        Watched {
            rent,
            model: model.to_string(),
            told: BTreeSet::new(),
            due,
            tools_after: 0,
            at_top,
        }
    }

    /// Takes what the run did.
    pub fn see(&mut self, seen: &Seen) {
        match seen {
            Seen::Call(c) => {
                if !self.told.insert(c.id.clone()) {
                    return;
                }
                self.rent.take(c, &self.model);
                if self.due.is_none()
                    && !c.subagent
                    && let Some(Judged::Due { rent, buy }) = self.rent.judged()
                {
                    self.due = Some((self.told.len() as i64, rent, buy));
                }
            }
            Seen::Tool { .. } if self.due.is_some() => self.tools_after += 1,
            _ => {}
        }
    }

    /// What the round's record keeps under `$.engine.handover` once its session came due; none while it never did.
    pub fn record(&self) -> Option<Value> {
        let (due_at, rent, buy) = self.due?;
        Some(json!({
            "due_at": due_at,
            "rent": rent,
            "buy": buy,
            "at_top": self.at_top,
            "tools_after": self.tools_after,
        }))
    }

    /// The model a call of the session ran on that has no prices, by which the session is never judged.
    pub fn unpriced(&self) -> Option<String> {
        match self.rent.judged() {
            Some(Judged::NoPrices { model }) => Some(model),
            _ => None,
        }
    }
}

/// The hand-over cell's step: the cell that called it handed its progress over. Its session ends and a new one of its
/// line opens, which the cell's next round runs in, so that the line's next round is given the cell's whole input with
/// what was handed over. What was said to the round that handed over and is recorded as delivered in it waits again,
/// since that round answered none of it. Gives what was handed over, word for word, with the rent and the buy the
/// session came to, its first and last prompts and the model, by which a hand-over's cost can be looked at afterwards.
pub(crate) fn hand_over(e: &mut Engine, w: &Work, gathered: &[Gathered]) -> Result<Value, String> {
    let caller = w
        .pos
        .callers
        .last()
        .cloned()
        .ok_or_else(|| format!("work {}: no cell handed its progress over", w.id))?;
    let rounds = work::rounds(e.db.conn(), w.id)?;
    let from = rounds
        .iter()
        .rev()
        .find(|r| r.cell.as_deref() == Some(caller.as_str()) && r.transition.as_deref() == Some(FORM))
        .ok_or_else(|| format!("work {}: no round of cell {caller} handed its progress over", w.id))?;
    let session = from
        .session
        .ok_or_else(|| format!("{}: handed its progress over in no session", from.id))?;
    let (line, cli, cli_session, model): (String, String, String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT s.line, s.cli, s.cli_session, (SELECT model FROM rounds WHERE work = ?2 AND n = ?3)
                 FROM sessions s WHERE s.id = ?1",
                db::rusqlite::params![session, from.id.work, from.id.n],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|x| x.to_string())?;
    let model = model.unwrap_or_default();
    let rent = sessions::handover::rent_of(e.db.conn(), &cli_session, &model)?;
    let costs = rent.judged().and_then(|j| j.costs());
    let handed = gathered.iter().find_map(|x| x.value.clone()).unwrap_or(Value::Null);
    let fresh = agent::new_session_id();
    let round = from.id;
    e.db.write(Actor::Engine, |tx| {
        failed::put_back_said(tx, round)?;
        sessions::choose::open(tx, w.id, &line, &cli, &fresh, sessions::Start::Handover).map_err(db::Error::Abort)?;
        Ok(())
    })
    .map_err(|x| x.to_string())?;
    e.say(&format!(
        "{round}: cell {caller} handed its progress over; the {line}'s session ends, and the cell carries on in a new one"
    ));
    let (last_prompt, last_model) = rent.last.clone().unzip();
    Ok(json!({
        "handover": handed,
        "session": {
            "rent": costs.map(|c| c.0),
            "buy": costs.map(|c| c.1),
            "opening": rent.opening,
            "last_prompt": last_prompt,
            "model": last_model.unwrap_or(model),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sessions::cli::Call;

    fn call(id: &str, prompt: i64) -> Seen {
        Seen::Call(Call {
            id: id.into(),
            model: Some("claude-sonnet-5-5".into()),
            read: prompt,
            ..Call::default()
        })
    }

    fn tool() -> Seen {
        Seen::Tool {
            name: "Bash".into(),
            on: None,
            digest: 1,
        }
    }

    // A round's watch records the call its session came due at, by its number in the ledger, which counts a subagent's
    // calls and each call once however often it is told, and the tool uses after it; one never due records nothing.
    #[test]
    fn a_watch_records_the_call_its_session_came_due_at_and_the_tools_after() {
        let mut w = Watched::new(Rent::default(), "m", false);
        w.see(&call("a", 10_000));
        w.see(&call("a", 10_000));
        w.see(&Seen::Call(Call {
            id: "s".into(),
            subagent: true,
            read: 900_000,
            ..Call::default()
        }));
        for i in 0..27 {
            w.see(&call(&format!("c{i}"), 110_000));
            w.see(&tool());
        }
        assert_eq!(w.record(), None);
        // The 28th call past the opening is due: the 30th told, the subagent's among them.
        w.see(&call("due", 110_000));
        w.see(&tool());
        w.see(&call("after", 110_000));
        w.see(&tool());
        let r = w.record().unwrap();
        assert_eq!(
            (r["due_at"].as_i64(), r["tools_after"].as_i64()),
            (Some(30), Some(2)),
            "{r}"
        );
        assert_eq!(r["at_top"], false);
        assert!((r["rent"].as_f64().unwrap() - 0.28).abs() < 1e-9, "{r}");
        assert!((r["buy"].as_f64().unwrap() - 0.275).abs() < 1e-9, "{r}");
    }

    // A session due before the round's first call came due at 0, whatever the round's calls; one of a model with no
    // prices is never due, and says which.
    #[test]
    fn a_session_due_at_the_start_came_due_at_none_of_the_rounds_calls() {
        let mut before = Rent::default();
        before.take(
            &Call {
                id: "a".into(),
                read: 10_000,
                ..Call::default()
            },
            "claude-sonnet-5-5",
        );
        for i in 0..30 {
            before.take(
                &Call {
                    id: format!("c{i}"),
                    read: 110_000,
                    ..Call::default()
                },
                "claude-sonnet-5-5",
            );
        }
        let mut w = Watched::new(before, "claude-sonnet-5-5", true);
        w.see(&tool());
        let r = w.record().unwrap();
        assert_eq!(
            (r["due_at"].as_i64(), r["tools_after"].as_i64()),
            (Some(0), Some(1)),
            "{r}"
        );
        assert_eq!(r["at_top"], true);
        let mut unpriced = Watched::new(Rent::default(), "m", false);
        unpriced.see(&Seen::Call(Call {
            id: "a".into(),
            read: 10_000,
            ..Call::default()
        }));
        assert_eq!(unpriced.unpriced().as_deref(), Some("m"));
        assert_eq!(unpriced.record(), None);
    }
}
