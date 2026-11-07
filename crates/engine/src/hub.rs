//! The message hub, as the engine sends through it: what the engine tells of its own accord, and what an agent's return
//! tells the owner, each a message of the hub's tables (`db::send`) written in the transaction of what made it. A fault
//! of wf-agents goes to the flow's keeper and every other message to the owner. The engine's messages go to the
//! notifications log too, through the outbox; an agent's are logged by the action that sends them, in words that say
//! who said them.

use db::rusqlite::Transaction;
use db::{Message, Recipient, Sender};
use serde_json::json;

use crate::actions::outbox;
use crate::work::RoundId;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The engine's message, about a work and an ask of the owner when it is of one, sent to `to` and written to the
/// notifications log; gives the message.
fn engine_says(
    tx: &Transaction<'_>,
    work: Option<i64>,
    body: Option<serde_json::Value>,
    ask: Option<i64>,
    to: Recipient,
    text: &str,
) -> Result<i64, String> {
    let m = Message {
        sender: Sender::Engine,
        work,
        round: None,
        text,
        body,
        ask,
    };
    let id = db::send(tx, &m, to).map_err(sql)?;
    outbox(tx, work, "notify", &json!({ "text": text }))?;
    Ok(id)
}

/// Tells the owner something, about a work or about none.
pub fn tell(tx: &Transaction<'_>, work: Option<i64>, text: &str) -> Result<(), String> {
    engine_says(tx, work, None, None, Recipient::Owner, text).map(|_| ())
}

/// Tells the owner of a new ask of the owner, the message naming the ask for the owner's interface to link to.
pub fn tell_of_ask(tx: &Transaction<'_>, work: i64, ask: i64, text: &str) -> Result<(), String> {
    engine_says(tx, Some(work), None, Some(ask), Recipient::Owner, text).map(|_| ())
}

/// Tells the flow's keeper of a fault of wf-agents, the message naming its kind; gives the message.
pub fn fault(tx: &Transaction<'_>, work: i64, kind: &str, text: &str) -> Result<i64, String> {
    engine_says(
        tx,
        Some(work),
        Some(json!({ "kind": kind })),
        None,
        Recipient::Keeper,
        text,
    )
}

/// Tells the flow's keeper of a new ask of the owner that reports a fault of wf-agents, the message naming the kind of
/// fault and the ask.
pub fn fault_ask(tx: &Transaction<'_>, work: i64, ask: i64, kind: &str, text: &str) -> Result<(), String> {
    engine_says(
        tx,
        Some(work),
        Some(json!({ "kind": kind })),
        Some(ask),
        Recipient::Keeper,
        text,
    )
    .map(|_| ())
}

/// What an agent's return tells the owner, as the message of the round that holds it, from the line of an engineer's
/// work or of an architect's; `design_card` the decision card the owner's question it answers was about. The action
/// that sends it writes it to the notifications log.
pub fn agent_tells(
    tx: &Transaction<'_>,
    sender: Sender,
    round: RoundId,
    text: &str,
    design_card: Option<i64>,
) -> Result<i64, String> {
    let m = Message {
        sender,
        work: Some(round.work),
        round: Some(round.n),
        text,
        body: design_card.map(|c| json!({ "design_card": c })),
        ask: None,
    };
    db::send(tx, &m, Recipient::Owner).map_err(sql)
}
