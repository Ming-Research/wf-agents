//! What rows and cells do besides moving the work: `engine.actions` in the flow. Each runs inside the transaction
//! that ends the round, so it happens exactly when the round is recorded. What must happen outside the database (a
//! message to the owner, closing a PR, reverting a merge, removing a worktree) is put in the outbox in that
//! transaction and carried out by [`run_outbox`] after it. What the flow sends to the architect is queued for it as a
//! matter, in the same transaction; the actions of the architect's own graph are [`crate::architect::act`].

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use db::rusqlite::{OptionalExtension, Transaction, params};
use db::{Matter, queue_matter};
use flow::CompiledCell;
use serde_json::{Map, Value, json};

use crate::inputs::at_path;
use crate::work::{Item, RoundId, Work, json_col, query_json};
use crate::{Engine, git};

/// What an action knows: the work as it was when the round started, the round and its return.
pub struct Ctx {
    pub work: Work,
    pub item: Option<Item>,
    pub project: Option<config::Project>,
    /// The CLI the item's level runs on.
    pub cli: Option<String>,
    pub round: RoundId,
    pub cell: CompiledCell,
    pub output: Value,
    /// The return as rows read it: an agent's form taken out of the field it sits in.
    pub unwrapped: Value,
    /// The field an agent's forms sit in.
    pub within: String,
    pub data: PathBuf,
    /// What the cell's rows were given: the values rows recorded, the engine's values, and each input read by id under
    /// `@<id>`.
    pub given: Map<String, Value>,
    /// The configuration's projects, for the works an item change ends.
    pub projects: BTreeMap<String, config::Project>,
    /// The line of the cell that called the cell the round is at, if one waits for it: the agent whose answers a cell
    /// that hands them on hands on.
    pub caller_line: Option<String>,
    /// The row the return takes, and where it takes the work.
    pub row: flow::CompiledRow,
    pub to: flow::Position,
    /// The cells of the work's graph where a stopped task stands while its blocker is held.
    pub blockers: Option<flow::format::Blockers>,
}

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Puts an action in the outbox, due now.
pub(crate) fn outbox(tx: &Transaction<'_>, work: Option<i64>, kind: &str, args: &Value) -> Result<(), String> {
    tx.execute(
        "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, ?2, ?3, ?4)",
        params![work, kind, args.to_string(), db::now()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Tells the owner something: kept for the owner's interface in the same transaction, and written to the
/// notifications log through the outbox.
pub fn notify(tx: &Transaction<'_>, work: Option<i64>, text: &str) -> Result<(), String> {
    tell(tx, work, None, None, text).map(|_| ())
}

/// Tells the owner of a new ask of the owner, the notice naming the ask for the owner's interface to link to.
pub fn notify_ask(tx: &Transaction<'_>, work: i64, ask: i64, text: &str) -> Result<(), String> {
    tell(tx, Some(work), Some(ask), None, text).map(|_| ())
}

/// Tells the owner of a new ask of the owner that reports a fault of wf-agents, the notice naming the ask and the kind
/// of fault.
pub fn notify_fault_ask(tx: &Transaction<'_>, work: i64, ask: i64, fault: &str, text: &str) -> Result<(), String> {
    tell(tx, Some(work), Some(ask), Some(fault), text).map(|_| ())
}

/// Tells the owner of a fault of wf-agents, the notice naming its kind for the owner's interface to set it apart;
/// gives the notice.
pub fn notify_fault(tx: &Transaction<'_>, work: i64, fault: &str, text: &str) -> Result<i64, String> {
    tell(tx, Some(work), None, Some(fault), text)
}

fn tell(
    tx: &Transaction<'_>,
    work: Option<i64>,
    ask: Option<i64>,
    fault: Option<&str>,
    text: &str,
) -> Result<i64, String> {
    tx.execute(
        "INSERT INTO engine_notices (work, ask, fault, text, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![work, ask, fault, text, db::now()],
    )
    .map_err(sql)?;
    let id = tx.last_insert_rowid();
    outbox(tx, work, "notify", &json!({ "text": text }))?;
    Ok(id)
}

impl Ctx {
    /// What an action is carried out at: a place of the return, or an input the cell read by id (`@<id>`).
    pub fn at(&self, path: &str) -> Option<Value> {
        if path.starts_with('@') {
            return self.given.get(path).cloned();
        }
        at_path(&self.unwrapped, path)
    }

    fn about(&self) -> String {
        match &self.item {
            Some(i) => format!("work {} (item {}: {})", self.work.id, i.id, i.title),
            None => format!(
                "work {} (a wake of the architect of {})",
                self.work.id,
                self.work.project.as_deref().unwrap_or("no project")
            ),
        }
    }

    fn item_id(&self) -> Result<i64, String> {
        self.item
            .as_ref()
            .map(|i| i.id)
            .ok_or_else(|| format!("work {} has no item", self.work.id))
    }

    fn project(&self) -> Result<&config::Project, String> {
        self.project
            .as_ref()
            .ok_or_else(|| format!("work {}: its project is not in the configuration", self.work.id))
    }

    /// The last accepted return of a cell of this work that has a value at a path, with its round.
    fn last(&self, tx: &Transaction<'_>, cell: &str, path: &str) -> Result<Option<(RoundId, Value)>, String> {
        let p = format!("$.{path}");
        let work = self.work.id;
        tx.query_row(
            "SELECT n, json_extract(output, ?3) FROM rounds
             WHERE work = ?1 AND cell = ?2 AND transition IS NOT NULL AND json_extract(output, ?3) IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            params![work, cell, p],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, db::rusqlite::types::Value>(1)?)),
        )
        .optional()
        .map_err(sql)
        .map(|o| o.map(|(n, v)| (RoundId::new(work, n), sql_value(v))))
    }
}

/// A value json_extract gave, as JSON: objects and lists come back as their text.
fn sql_value(v: db::rusqlite::types::Value) -> Value {
    use db::rusqlite::types::Value as V;
    match v {
        V::Null => Value::Null,
        V::Integer(i) => json!(i),
        V::Real(f) => json!(f),
        V::Text(t) => serde_json::from_str(&t).unwrap_or(Value::String(t)),
        V::Blob(_) => Value::Null,
    }
}

/// Whether the design paths changed on the work's branch against main.
fn tree_changed(w: &Work, project: &config::Project) -> Result<bool, String> {
    let Some(wt) = &w.worktree else { return Ok(false) };
    let fork = git::fork_point(wt, &project.main)?;
    Ok(git::changed(wt, &fork, "HEAD")?
        .iter()
        .any(|f| git::under(f, &project.design_paths)))
}

/// The work's design cards not yet replaced, newest first: their ids and statuses.
fn current_design_cards(tx: &Transaction<'_>, work: i64) -> Result<Vec<(i64, String)>, String> {
    let mut st = tx
        .prepare("SELECT id, status FROM design_cards WHERE work = ?1 AND status <> 'replaced' ORDER BY id DESC")
        .map_err(sql)?;
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// The work's design card not yet replaced: its id and status.
fn current_design_card(tx: &Transaction<'_>, work: i64) -> Result<Option<(i64, String)>, String> {
    tx.query_row(
        "SELECT id, status FROM design_cards WHERE work = ?1 AND status <> 'replaced' ORDER BY id DESC LIMIT 1",
        [work],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(sql)
}

/// A new design card of a round, open, with what the round said changed from the cards it replaces.
fn new_design_card(tx: &Transaction<'_>, round: RoundId, body: &Value, changed: Option<&Value>) -> Result<i64, String> {
    let changed = changed.and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    tx.execute(
        "INSERT INTO design_cards (work, round, body, status, changed) VALUES (?1, ?2, ?3, 'open', ?4)",
        params![round.work, round.n, body.to_string(), changed],
    )
    .map_err(sql)?;
    Ok(tx.last_insert_rowid())
}

/// Queues a new design card for the architect of the work's project to look at: whether it holds and the approaches
/// it chose are reasonable. The owner alone rules on it.
fn design_card_matter(ctx: &Ctx, tx: &Transaction<'_>, card: i64, raised: RoundId) -> Result<(), String> {
    let Some(item) = &ctx.item else { return Ok(()) };
    let what = format!("Design card {card} of {} to look at", ctx.about());
    let m = Matter {
        project: item.project.clone(),
        kind: "design_card".into(),
        what,
        body: json!({ "design_card": card }),
        item: Some(item.id),
        raised: Some((raised.work, raised.n)),
        ..Matter::default()
    };
    queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

/// Carries out one action.
pub fn apply(ctx: &Ctx, tx: &Transaction<'_>, name: &str, at: Option<&str>) -> Result<(), String> {
    let w = ctx.work.id;
    let at_value = at.and_then(|p| ctx.at(p));
    if crate::architect::act(ctx, tx, name, at_value.as_ref())?
        || crate::blockers::act(ctx, tx, name, at_value.as_ref())?
    {
        return Ok(());
    }
    match name {
        "record_place" => {
            let p = &ctx.output["place"];
            tx.execute(
                "UPDATE works SET machine = ?2, worktree = ?3, branch = ?4, pr = coalesce(?5, pr) WHERE id = ?1",
                params![
                    w,
                    p["machine"].as_str(),
                    p["worktree"].as_str(),
                    p["branch"].as_str(),
                    p["pr"].as_i64()
                ],
            )
            .map_err(sql)?;
            if let (Some(cli), Some(s)) = (&ctx.cli, p["session"].as_str()) {
                sessions::choose::begin(tx, w, crate::ENGINEER, cli, s)?;
            }
        }
        "record_tool_runs" => {
            let runs = query_json(
                tx,
                "SELECT result FROM tool_runs WHERE work = ?1 AND round = ?2 AND tool = 'ci' ORDER BY id",
                &[&ctx.round.work, &ctx.round.n],
                1000,
            )?;
            let mut head: Option<String> = None;
            let mut pr: Option<i64> = None;
            for r in runs {
                let res = &r["result"];
                if res["push"]["verdict"] == "pushed" {
                    head = res["push"]["head"].as_str().map(str::to_string);
                }
                if let Some(n) = res["pr"].as_i64() {
                    pr = Some(n);
                }
            }
            tx.execute(
                "UPDATE works SET pushed_head = coalesce(?2, pushed_head), pr = coalesce(?3, pr) WHERE id = ?1",
                params![w, head, pr],
            )
            .map_err(sql)?;
        }
        "lock_acceptance" => {
            let Some((round, acceptance)) = ctx.last(tx, "D2", &format!("{}.acceptance", ctx.within))? else {
                return Err("no design handed in acceptance conditions to lock".into());
            };
            let item = ctx.item_id()?;
            let locked: Option<String> = tx
                .query_row("SELECT acceptance FROM items WHERE id = ?1", [item], |r| r.get(0))
                .map_err(sql)?;
            if json_col(locked).as_ref() != Some(&acceptance) {
                tx.execute(
                    "UPDATE items SET acceptance = ?2, acceptance_work = ?3, acceptance_round = ?4 WHERE id = ?1",
                    params![item, acceptance.to_string(), round.work, round.n],
                )
                .map_err(sql)?;
            }
        }
        "lock_new_acceptance" => {
            // No conditions at all is not new acceptance: the reply left them as they are.
            let Some(acceptance) = at_value
                .or_else(|| at_path(&ctx.unwrapped, "reply.new_acceptance"))
                .filter(|a| a.as_array().is_none_or(|xs| !xs.is_empty()))
            else {
                return Ok(());
            };
            tx.execute(
                "UPDATE items SET acceptance = ?2, acceptance_work = ?3, acceptance_round = ?4 WHERE id = ?1",
                params![ctx.item_id()?, acceptance.to_string(), ctx.round.work, ctx.round.n],
            )
            .map_err(sql)?;
        }
        "post_design_card" => {
            if !tree_changed(&ctx.work, ctx.project()?)? {
                return Ok(());
            }
            let path = format!("{}.design_tree.design_card", ctx.within);
            let Some((round, body)) = ctx.last(tx, "D2", &path)? else {
                return Ok(());
            };
            let posted: Option<i64> = tx
                .query_row(
                    "SELECT id FROM design_cards WHERE work = ?1 AND round = ?2",
                    [round.work, round.n],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            if posted.is_some() {
                return Ok(());
            }
            // What changed from the cards a redesign set aside is said beside the card, in the same return.
            let changed: Option<Value> = tx
                .query_row(
                    "SELECT json_extract(output, ?3) FROM rounds WHERE work = ?1 AND n = ?2",
                    params![
                        round.work,
                        round.n,
                        format!("$.{}.design_tree.design_card_changed", ctx.within)
                    ],
                    |r| r.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(sql)?
                .flatten()
                .map(Value::String);
            let card = new_design_card(tx, round, &body, changed.as_ref())?;
            // The cards of this work the redesign set aside are replaced by this one.
            tx.execute(
                "UPDATE design_cards SET replaced_by = ?2
                 WHERE status = 'replaced' AND replaced_by IS NULL AND id <> ?2 AND work = ?1",
                params![w, card],
            )
            .map_err(sql)?;
            design_card_matter(ctx, tx, card, round)?;
            notify(
                tx,
                Some(w),
                &format!(
                    "Design card {card} of {} waits for your ruling; the architect looks at it too. It blocks nothing until the merge.",
                    ctx.about()
                ),
            )?;
        }
        "update_design_card" => {
            let Some(body) = at_value else { return Ok(()) };
            // The cards it replaces are read before it is made: read after, the newest not replaced is itself.
            let before = current_design_cards(tx, w)?;
            let changed = at_path(&ctx.unwrapped, "design_card_changed");
            let card = new_design_card(tx, ctx.round, &body, changed.as_ref())?;
            design_card_matter(ctx, tx, card, ctx.round)?;
            if before.is_empty() {
                notify(tx, Some(w), &format!("Design card {card} of {} is new.", ctx.about()))?;
            } else {
                for (old, _) in &before {
                    tx.execute(
                        "UPDATE design_cards SET status = 'replaced', replaced_by = ?2 WHERE id = ?1",
                        params![old, card],
                    )
                    .map_err(sql)?;
                }
                let again = if before.iter().any(|(_, status)| status == "ruled") {
                    " A decision you ruled on changed: it must be confirmed again."
                } else {
                    ""
                };
                let olds = before
                    .iter()
                    .map(|(id, _)| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                notify(
                    tx,
                    Some(w),
                    &format!(
                        "Design card {card} of {} replaces design card {olds}.{again}",
                        ctx.about()
                    ),
                )?;
            }
        }
        "replace_design_card" => {
            tx.execute(
                "UPDATE design_cards SET status = 'replaced' WHERE status IN ('open', 'ruled') AND work = ?1",
                [w],
            )
            .map_err(sql)?;
        }
        "notify_pending_approvers" => {
            let why = match current_design_card(tx, w)? {
                Some((card, status)) if status == "open" => format!("design card {card} waits for your ruling"),
                Some((card, _)) => format!("design card {card} is ruled, but not confirmed for the head to merge"),
                None => "it waits for the items it depends on to merge".to_string(),
            };
            notify(tx, Some(w), &format!("{} is ready to merge, but {why}.", ctx.about()))?;
        }
        "revert_merge" => {
            let project = ctx.project()?;
            let Some((_, merged)) = ctx.last(tx, "M2", "result.merge_commit")? else {
                return Err("no merge to revert".into());
            };
            outbox(
                tx,
                Some(w),
                "revert_merge",
                &json!({
                    "repo": project.repo,
                    "main": project.main,
                    "checkout": project.checkout,
                    "merge_commit": merged,
                    "worktrees": config::worktrees(&ctx.data),
                }),
            )?;
        }
        "reopen_with_fix" => {
            // The merged PR is closed; the branch carries on, and its next push opens a new PR with the fix.
            tx.execute("UPDATE works SET pr = NULL WHERE id = ?1", [w])
                .map_err(sql)?;
        }
        "report_missing_check" => {
            let names: Vec<String> = at_path(&ctx.unwrapped, "main.checks.name")
                .map(|v| match v {
                    Value::Array(xs) => xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    Value::String(s) => vec![s],
                    _ => Vec::new(),
                })
                .unwrap_or_default();
            notify(
                tx,
                Some(w),
                &format!(
                    "Main broke after {} merged, and the full CI on the draft did not catch it ({}). The merge is being reverted; add the check that broke main to the project's full CI.",
                    ctx.about(),
                    if names.is_empty() {
                        "the deployment".to_string()
                    } else {
                        names.join(", ")
                    }
                ),
            )?;
        }
        "record_waits" => {
            let Some((_, waits)) = ctx.last(tx, "A2", "reply.waits")? else {
                return Ok(());
            };
            let item = ctx.item_id()?;
            for x in waits.as_array().cloned().unwrap_or_default() {
                let Some(key) = x["key"]
                    .as_str()
                    .and_then(|k| k.trim_start_matches('#').parse::<i64>().ok())
                else {
                    return Err(format!(
                        "the reply has the work wait for {}, which is not an item number",
                        x["key"]
                    ));
                };
                let exists: Option<i64> = tx
                    .query_row(
                        "SELECT 1 FROM item_links WHERE item = ?1 AND depends_on = ?2",
                        params![item, key],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(sql)?;
                // The work started before what it waits for: a parallel dependency, which stops no start.
                if exists.is_none() {
                    let needs = x["delivers"].as_str().unwrap_or_default();
                    db::add_dependency(tx, item, key, needs, "parallel").map_err(sql)?;
                }
            }
        }
        "hand_over_to_sub_items" => {
            let Some(pr) = ctx.work.pr else { return Ok(()) };
            let children = ctx
                .last(tx, "A2", "reply.children")?
                .map(|(_, v)| v)
                .unwrap_or(Value::Null);
            let list: Vec<String> = children
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|c| {
                    format!(
                        "#{} {}",
                        c["key"].as_str().unwrap_or("?"),
                        c["title"].as_str().unwrap_or_default()
                    )
                })
                .collect();
            let comment = format!("Sub-items took over this work: {}. It does not merge.", list.join(", "));
            outbox(
                tx,
                Some(w),
                "close_pr",
                &json!({ "repo": ctx.project()?.repo, "pr": pr, "comment": comment }),
            )?;
        }
        // The session manager opens the new session as the round at the cell the row leads to starts, at the work's
        // first round there: the cell's `new_session`, which the compiler sets from this action.
        "open_session" => {}
        "wake_architect" => {
            // Each side report is one matter of the kind report for the architect of the work's project, queued once,
            // which wakes it when no wake of it runs; its queue marks one urgent.
            let Some(item) = &ctx.item else { return Ok(()) };
            let reports = ctx.unwrapped["side_reports"].as_array().cloned().unwrap_or_default();
            for (index, r) in reports.iter().enumerate() {
                let queued: i64 = tx
                    .query_row(
                        "SELECT count(*) FROM architect_matters
                         WHERE work = ?1 AND round = ?2 AND json_extract(body, '$.index') = ?3",
                        params![ctx.round.work, ctx.round.n, index as i64],
                        |x| x.get(0),
                    )
                    .map_err(sql)?;
                // A report the work handed in before, word for word, is not queued again.
                let said = r["what"].as_str().unwrap_or_default().trim();
                let before: i64 = tx
                    .query_row(
                        "SELECT count(*) FROM architect_matters WHERE work = ?1 AND kind = 'report'
                           AND trim(json_extract(body, '$.report.what'), char(32, 9, 10, 13)) = ?2",
                        params![ctx.round.work, said],
                        |x| x.get(0),
                    )
                    .map_err(sql)?;
                if queued > 0 || before > 0 {
                    continue;
                }
                let urgent = r["urgent"] == true;
                let what = format!(
                    "A side report from {} at cell {}: {}",
                    ctx.about(),
                    ctx.cell.id,
                    db::one_sentence(r["what"].as_str().unwrap_or_default())
                );
                let m = Matter {
                    project: item.project.clone(),
                    kind: "report".into(),
                    detail: r["kind"].as_str().map(str::to_string),
                    what,
                    body: json!({ "report": r, "index": index, "cell": ctx.cell.id, "urgent": urgent }),
                    item: Some(item.id),
                    raised: Some((ctx.round.work, ctx.round.n)),
                };
                queue_matter(tx, &m).map_err(sql)?;
            }
        }
        "deliver_answers" => {
            // At the cell that hands on the answers of the cell that called it, that cell's; else the return's own:
            // its answers, or those a redirect carries to the speakers it does not redirect for.
            let (line, all) = match at_value.as_ref().and_then(Value::as_array) {
                Some(xs) if at.is_some_and(|p| p.starts_with('@')) => {
                    (ctx.caller_line.clone().unwrap_or_else(|| line_of(ctx)), xs.clone())
                }
                _ => {
                    let mut all = ctx.unwrapped["answers"].as_array().cloned().unwrap_or_default();
                    all.extend(
                        ctx.unwrapped["redirect"]["answers"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default(),
                    );
                    (line_of(ctx), all)
                }
            };
            // The cell whose answers they are: the one that called, for the cell that hands them on.
            let cell = match at {
                Some(p) if p.starts_with('@') => ctx.work.pos.callers.last().cloned(),
                _ => None,
            }
            .unwrap_or_else(|| ctx.cell.id.clone());
            for a in all {
                let to = a["to"].as_str().unwrap_or("owner");
                let text = a["text"].as_str().unwrap_or_default();
                // An answer to the architect reaches its queue, as a report it takes in.
                if to == "architect"
                    && let Some(item) = &ctx.item
                {
                    let m = Matter {
                        project: item.project.clone(),
                        kind: "report".into(),
                        detail: Some("answer".into()),
                        what: format!(
                            "The {line} of {} at cell {cell} answers what you said: {}",
                            ctx.about(),
                            db::one_sentence(text)
                        ),
                        body: json!({
                            "report": { "kind": "answer", "what": text, "urgent": false },
                            "cell": cell,
                            "line": line,
                        }),
                        item: Some(item.id),
                        raised: Some((ctx.round.work, ctx.round.n)),
                    };
                    queue_matter(tx, &m).map_err(sql)?;
                    continue;
                }
                notify(
                    tx,
                    Some(w),
                    &format!("The {line} of {} answers the {to}: {text}", ctx.about()),
                )?;
            }
        }
        "tell_owner" => {
            let said: Vec<String> = ctx.unwrapped["tell_owner"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            // The owner's interface shows the message as the agent's, with its cell; the notifications log says it too,
            // and no notice of the engine's repeats it.
            for text in said {
                tx.execute(
                    "INSERT INTO owner_messages (work, round, text) VALUES (?1, ?2, ?3)",
                    params![ctx.round.work, ctx.round.n, text],
                )
                .map_err(sql)?;
                let logged = format!(
                    "The {} of {} at cell {} tells you: {text}",
                    line_of(ctx),
                    ctx.about(),
                    ctx.cell.id
                );
                outbox(tx, Some(w), "notify", &json!({ "text": logged }))?;
            }
        }
        "queue_flow_feedback" => {
            // What the round said was wrong with its cell joins its project's architect's flow feedback, by cell, which
            // wakes no architect; none is nothing to add.
            let said = at_value.as_ref().and_then(Value::as_str).unwrap_or_default().trim();
            if crate::architect::says_none(said) {
                return Ok(());
            }
            let project = match (&ctx.item, &ctx.work.project) {
                (Some(i), _) => i.project.clone(),
                (None, Some(p)) => p.clone(),
                (None, None) => return Ok(()),
            };
            let entry = json!({
                "work": ctx.round.work,
                "round": ctx.round.n,
                "item": ctx.item.as_ref().map(|i| i.id),
                "said": said,
            });
            crate::architect::add_flow_feedback(tx, &project, &ctx.work.graph, &ctx.cell.id, entry)?;
        }
        other => return Err(format!("no such engine action: {other}")),
    }
    Ok(())
}

/// The line of the cell an action is carried out for: its own, or the line of its graph's agents.
fn line_of(ctx: &Ctx) -> String {
    match &ctx.cell.line {
        Some(l) => l.clone(),
        None if ctx.work.graph == crate::ARCHITECT => crate::ARCHITECT.to_string(),
        None => crate::ENGINEER.to_string(),
    }
}

/// Carries out what is due in the outbox; an action that fails is tried again later, waiting longer each time.
/// Gives whether anything was done.
pub fn run_outbox(e: &mut Engine) -> Result<bool, String> {
    let due = query_json(
        e.db.conn(),
        "SELECT id, work, kind, args, attempts FROM outbox WHERE run_after <= ?1 ORDER BY id",
        &[&db::now()],
        100,
    )?;
    let mut did = false;
    for row in due {
        let id = row["id"].as_i64().unwrap_or_default();
        let args = &row["args"];
        let result = match row["kind"].as_str().unwrap_or_default() {
            "notify" => notify_owner(e, args["text"].as_str().unwrap_or_default()),
            "remove_worktree" => {
                let checkout = PathBuf::from(args["checkout"].as_str().unwrap_or_default());
                git::remove_worktree(&checkout, Path::new(args["dir"].as_str().unwrap_or_default()))
            }
            "close_pr" => e
                .github
                .close(
                    args["repo"].as_str().unwrap_or_default(),
                    args["pr"].as_i64().unwrap_or_default(),
                    args["comment"].as_str().unwrap_or_default(),
                )
                .map_err(|f| f.what),
            "revert_merge" => revert(e, args),
            other => Err(format!("no such outbox action: {other}")),
        };
        did = true;
        match result {
            Ok(()) => {
                e.db.conn()
                    .execute("DELETE FROM outbox WHERE id = ?1", [id])
                    .map_err(sql)?;
            }
            Err(why) => {
                let n = row["attempts"].as_i64().unwrap_or_default() + 1;
                let wait = (60_000_i64 << n.min(6)).min(3_600_000);
                (e.say)(&format!("outbox {id} ({}) failed, attempt {n}: {why}", row["kind"]));
                e.db.conn()
                    .execute(
                        "UPDATE outbox SET attempts = ?2, last_error = ?3, run_after = ?4 WHERE id = ?1",
                        params![id, n, why, db::now() + wait],
                    )
                    .map_err(sql)?;
            }
        }
    }
    Ok(did)
}

/// What the owner is told is kept in the data directory's notifications.log, and said where the engine says things.
fn notify_owner(e: &Engine, text: &str) -> Result<(), String> {
    let path = e.data.join("notifications.log");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|x| format!("{}: {x}", path.display()))?;
    writeln!(f, "{} {}", db::now(), text.replace('\n', " ")).map_err(|x| format!("{}: {x}", path.display()))?;
    (e.say)(&format!("to the owner: {text}"));
    Ok(())
}

/// Opens a PR that reverts a merge, and merges it.
fn revert(e: &Engine, args: &Value) -> Result<(), String> {
    let s = |k: &str| args[k].as_str().unwrap_or_default().to_string();
    let (main, sha) = (s("main"), s("merge_commit"));
    let checkout = PathBuf::from(s("checkout"));
    let short: String = sha.chars().take(10).collect();
    let branch = format!("wf-agents/revert-{short}");
    let dir = PathBuf::from(s("worktrees")).join(format!("revert-{short}"));
    git::git(&checkout, &["fetch", "-q", "origin", &main])?;
    git::remove_worktree(&checkout, &dir)?;
    git::git(
        &checkout,
        &[
            "worktree",
            "add",
            "-q",
            "-B",
            &branch,
            &dir.to_string_lossy(),
            &format!("origin/{main}"),
        ],
    )?;
    let result = revert_in(e, &dir, &s("repo"), &main, &sha, &branch);
    let _ = git::remove_worktree(&checkout, &dir);
    result
}

fn revert_in(e: &Engine, dir: &Path, repo: &str, main: &str, sha: &str, branch: &str) -> Result<(), String> {
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    git::git(dir, &[&who[..], &["revert", "--no-edit", sha]].concat())?;
    let head = git::head(dir)?;
    crate::hub::push(dir, &head, &format!("refs/heads/{branch}"), true).map_err(|f| f.what)?;
    let short: String = sha.chars().take(10).collect();
    let title = format!("Revert {short}: it broke main");
    let body = format!("The merge {sha} broke main. This reverts it so that main recovers; the fix comes in a new PR.");
    let pr = e
        .github
        .pull_request(repo, branch, main, Some((&title, &body)), None)
        .map_err(|f| f.what)?;
    e.github.merge(repo, pr).map_err(|f| f.what)?;
    Ok(())
}
