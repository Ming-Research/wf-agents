//! What installing a flow would do with the works that go on, as the page reads it: the engine's own judgment
//! ([`engine::flow_cost`], which `wf-agents flow cost` prints too) in the interface's types, the works in the order the
//! command lists them.

use api::{CostEffect, FlowCost, PausedBy, WorkCost, WorkStands};
use engine::flow_cost::{Cost, Effect};

/// The cost of installing a flow, as the interface hands it out.
pub fn flow_cost(cost: &Cost) -> FlowCost {
    // The engine gives the works by id; the sort keeps that order within a group.
    let mut works: Vec<&engine::flow_cost::WorkCost> = cost.works.iter().collect();
    works.sort_by_key(|c| c.group());
    FlowCost {
        version: cost.version,
        stored: cost.stored,
        works: works.into_iter().map(work).collect(),
    }
}

fn work(c: &engine::flow_cost::WorkCost) -> WorkCost {
    WorkCost {
        work: c.id,
        stands: c.work.as_ref().map(|w| WorkStands {
            item: w.item,
            title: c.title.clone(),
            project: w.project.clone(),
            graph: w.graph.clone(),
            version: w.flow_version,
            at: w.pos.at.clone(),
            paused: paused(w),
        }),
        effect: effect(&c.effect),
    }
}

/// Who paused the work, as the command line says it: the architect's stop is the engine's pause by its reason.
fn paused(w: &engine::Work) -> Option<PausedBy> {
    let paused = w.paused.as_deref()?;
    let by = w.paused_by.as_deref();
    Some(if db::stopped_by_architect(Some(paused), by) {
        PausedBy::Architect
    } else if by == Some("owner") {
        PausedBy::Owner
    } else {
        PausedBy::Engine
    })
}

fn effect(e: &Result<Effect, String>) -> CostEffect {
    match e {
        Err(why) => CostEffect::Unreadable { why: why.clone() },
        Ok(Effect::Stays) => CostEffect::Stays,
        Ok(Effect::Moves) => CostEffect::Moves,
        Ok(Effect::GoesBack {
            through,
            to,
            why,
            from,
            redone,
        }) => CostEffect::GoesBack {
            through: through.clone(),
            to: to.clone(),
            why: why.clone(),
            from: from.as_ref().map(|r| r.n),
            redone: i64::try_from(*redone).unwrap_or(i64::MAX),
        },
        Ok(Effect::Stuck {
            at,
            why,
            back_to,
            pause,
        }) => CostEffect::Stuck {
            at: at.clone(),
            why: why.clone(),
            back_to: back_to.clone(),
            pause: pause.clone(),
        },
        Ok(Effect::Ends { why, taken_over }) => CostEffect::Ends {
            why: why.clone(),
            taken_over: *taken_over,
        },
    }
}
