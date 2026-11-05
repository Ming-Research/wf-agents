//! What the architect's changes to items may hold, and the rows of the engineer's graph the view of the ways works went
//! reads. No mark on an item sends its works one way or another: the research of each work judges that, so an item is
//! created and edited with its text alone, and the way a work went is read from the rows it took.

const FLOW: &str = include_str!("../../../flow/flow.json");

fn compiled() -> flow::Compiled {
    let context = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    flow::compile(FLOW, &context).unwrap_or_else(|e| panic!("{e}"))
}

// No cell of the architect's graph that takes changes to items lets a change create an item with a `route`, or edit
// the `route` of one: a return that did would fit the schema and be taken. Before the change `create_item` had the
// optional field `route` and `edit_item` the part `route`, so both appeared in the schema of every cell that takes the
// changes.
#[test]
fn no_change_to_an_item_names_a_route() {
    let compiled = compiled();
    let architect = compiled.graph("architect").unwrap();
    let taking: Vec<_> = architect
        .cells
        .iter()
        .filter(|cell| {
            cell.schema
                .as_ref()
                .is_some_and(|s| s.to_string().contains("\"create_item\""))
        })
        .collect();
    assert!(
        !taking.is_empty(),
        "no cell of the architect's graph takes changes to items"
    );
    for cell in taking {
        let schema = cell.schema.as_ref().unwrap().to_string();
        assert!(schema.contains("\"edit_item\""), "{}", cell.id);
        assert!(!schema.contains("\"route\""), "{} still names a route", cell.id);
    }
}

// The view `work_routes` reads the engineer's graph by the names it has: the research (`R1`) takes `start` to the
// implementation and `cannot_start` to the design (`D1`), and the implementation (`I1`) has the row `shallow_research`,
// which counts the counter of that name and goes back to the research, the one way to deep research. A graph that renamed any of them would leave the view
// giving no way, with no test of the view's own to notice, since those use the names as the view does.
#[test]
fn the_rows_the_view_of_the_ways_works_went_reads_are_in_the_engineers_graph() {
    let compiled = compiled();
    let engineer = compiled.graph("engineer").unwrap();
    let row = |cell: &str, name: &str| {
        engineer
            .cell(cell)
            .unwrap_or_else(|| panic!("no cell {cell}"))
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{cell} has no row {name}"))
    };
    assert_eq!(row("R1", "start").to, "I1");
    assert_eq!(row("R1", "cannot_start").to, "D1");
    let turn = row("I1", "shallow_research");
    assert_eq!(turn.to, "R1");
    assert_eq!(turn.counts.as_deref(), Some("shallow_research"));
}
