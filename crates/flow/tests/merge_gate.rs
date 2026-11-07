//! The cells past the merge gate of the engineer's graph, which the database names by hand
//! (`db::sql_past_gate_cells!`) to refuse a dependency of an item whose work is there on one not done, are those the flow
//! has.

use std::collections::BTreeSet;

fn compiled() -> flow::Compiled {
    let cx = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    flow::compile(flow::FLOW, &cx).unwrap_or_else(|e| panic!("{e}"))
}

/// A graph's merge gate: the one cell where a work waits for the items its item depends on.
fn gate(g: &flow::CompiledGraph) -> String {
    let gates: Vec<&str> = g
        .blockers
        .iter()
        .flat_map(|b| &b.cells)
        .filter(|c| c.waits && c.holders.iter().any(|h| h == "items"))
        .map(|c| c.cell.as_str())
        .collect();
    match gates[..] {
        [one] => one.to_string(),
        _ => panic!("{}: the cells that wait for the items are {gates:?}", g.name),
    }
}

/// The cells of the graph's own way a row of `cell` takes a work to: not a pointer such as `@stopped`, a cell of the
/// asking part or one that answers whoever called it, where the work stands for the cell it stopped at or that called,
/// and goes back to.
fn next<'g>(g: &'g flow::CompiledGraph, cell: &str) -> Vec<&'g str> {
    let Some(c) = g.cell(cell) else {
        return Vec::new();
    };
    c.rows
        .iter()
        .map(|r| r.to.as_str())
        .filter(|to| g.cell(to).is_some() && !g.asking.iter().any(|a| a == to) && !g.is_called(to))
        .collect()
}

/// The cells reached from `from`, `from` included, by [`next`] without passing `gate`.
fn reached<'g>(g: &'g flow::CompiledGraph, from: &[&'g str], gate: &str) -> BTreeSet<&'g str> {
    let mut seen = BTreeSet::new();
    let mut todo = from.to_vec();
    while let Some(c) = todo.pop() {
        if c != gate && seen.insert(c) {
            todo.extend(next(g, c));
        }
    }
    seen
}

/// The cells of a graph past its merge gate: those a work reaches by the gate's way on without passing the gate again,
/// from which it reaches the end without passing it; the end, where the item is done, is none of them.
fn past_the_gate(g: &flow::CompiledGraph) -> BTreeSet<String> {
    let gate = gate(g);
    let on: Vec<&str> = g
        .cell(&gate)
        .unwrap()
        .rows
        .iter()
        .filter(|r| r.main)
        .map(|r| r.to.as_str())
        .collect();
    reached(g, &on, &gate)
        .into_iter()
        .filter(|c| *c != g.end && reached(g, &[*c], &gate).contains(g.end.as_str()))
        .map(str::to_string)
        .collect()
}

/// The cells the database names past the merge gate.
fn named() -> BTreeSet<String> {
    db::sql_past_gate_cells!()
        .split(',')
        .map(|c| c.trim().trim_matches('\'').to_string())
        .collect()
}

// A work past the merge gate merges, or has merged, and goes to the end with no cell waiting for an item its item comes
// to depend on, so the database refuses that dependency on an item not done as it refuses one of an item done
// (`db::add_dependency`), and names those cells by hand. They are the engineer's graph's: the merge, and the cells its
// merged work goes through to the end. The fix a failed merge goes back to is not one, as the gate waits again after
// it. The derivation is shown to take a new cell on the way from the merge to the end, and to leave one out whose rows
// lead back through the gate only.
#[test]
fn the_cells_past_the_merge_gate_are_the_flows() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    assert_eq!(past_the_gate(g), named());
    let gate = gate(g);
    let main_to = |g: &flow::CompiledGraph, cell: &str| -> String {
        g.cell(cell).unwrap().rows.iter().find(|r| r.main).unwrap().to.clone()
    };
    let merge = main_to(g, &gate);
    // A cell ZZ whose one row leads to `to`, which the merge's main row leads to when it is on the merged work's way, and
    // a row of the merge's own beside it leads to otherwise.
    let with_zz = |to: &str, on_the_way: bool| -> flow::Compiled {
        let mut c = c.clone();
        let g = c.graphs.iter_mut().find(|g| g.name == "engineer").unwrap();
        let mut extra = g.cell(&merge).unwrap().clone();
        extra.id = "ZZ".into();
        extra.rows.retain(|r| r.main);
        extra.rows[0].to = to.to_string();
        let cell = g.cells.iter_mut().find(|x| x.id == merge).unwrap();
        let main = cell.rows.iter().position(|r| r.main).unwrap();
        if on_the_way {
            cell.rows[main].to = "ZZ".into();
        } else {
            let mut beside = cell.rows[main].clone();
            beside.name = "zz".into();
            beside.main = false;
            beside.to = "ZZ".into();
            cell.rows.push(beside);
        }
        g.cells.push(extra);
        c
    };
    // On the way from the merge to the end, it is past the gate.
    let on = with_zz(&main_to(g, &merge), true);
    let mut more = named();
    more.insert("ZZ".into());
    assert_eq!(past_the_gate(on.graph("engineer").unwrap()), more);
    // Leading back to the gate only, it is not.
    let back = with_zz(&gate, false);
    assert_eq!(past_the_gate(back.graph("engineer").unwrap()), named());
}
