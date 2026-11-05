//! The cells of the flow as works and rounds name them. A work and its rounds keep the cells' ids of the flow version
//! the work ran on. Cells had numbers (the engineer's graph) and codes (the architect's) first, then names, then names
//! with their stage, and now codes by stage, as `T6`: the stage's letter and the cell's order in it. A reader of the
//! works and rounds of every version names a cell by every id it has had, but for an id that is now another cell's
//! code: the architect's first codes P1 to P11, N1, Q1, Q2, X1 and S1 were other cells, and so was the engineer's S0,
//! its start before the cell an item waits at to start took that code; a round of those versions at one of them reads
//! as the cell with that code now, so that no round of this version reads as another cell. The architect's first two
//! cells were W0 and W1 while their stage was named wake; they took the letter of its name now, start. The engineer's
//! design stage was numbered again in the order it runs, when the engine's choices of its way became cells of their
//! own: its D2 and D4 to D9 were other cells before, and a round of those versions at one of them reads as the cell
//! with that code now. It was numbered once more when the design tree left the flow: D1 to D3 are again the three cells
//! they were before the tree, its cells D5 to D7 are gone, and D8 to D15 became D4 to D11; a round of a version with the
//! tree at D1 to D11 reads as the cell with that code now, and one at D12 to D15 as the cell that code became. The
//! tree's review (T5) is gone too, and keeps its row, so that a round there still reads as itself. The architect's
//! batch stage has a fifth cell, B5, which records the owner's decisions between B1 and B2; it was numbered after B4 so
//! that no cell of a flow version before changes its code. When the architect came to work from a base and a session
//! forked from it for each matter, its start's understanding (S1), its pick (Q1), its batch (B1 to B5), the plan after a
//! batch (H10) and the talk of the groups (H12) left the flow; they keep their rows, H12 one of its own now, so that a
//! round there still reads as itself, and the cells of main's red CI (L1, L2) and of the second eye (G1, G2) took codes
//! no version gave another cell. When a report came to be taken in one round, its second cell (H5) left the flow; it
//! keeps its row. Slots free with nothing to start took H11, a code no version gave another cell.

/// Each cell, by its graph and its code, with the ids it had in the flow versions before: its number or first code,
/// its name, and its name with its stage, where each was another.
const BEFORE: [(&str, &str, &[&str]); 87] = [
    ("engineer", "S0", &["U0"]),
    ("engineer", "S1", &["0", "start", "S0"]),
    ("engineer", "G1", &[]),
    ("engineer", "R1", &["1", "research", "research.problem"]),
    ("engineer", "R2", &["1a", "stands", "research.stands"]),
    ("engineer", "G2", &[]),
    ("engineer", "D1", &["2", "compare", "design.compare"]),
    ("engineer", "D2", &["2a", "choose", "design.choose"]),
    ("engineer", "D3", &["2b", "rethink_design", "design.rethink"]),
    ("engineer", "D4", &[]),
    ("engineer", "D5", &["3a", "gap_under_test", "design.gap_under_test"]),
    ("engineer", "D6", &["3b", "sure", "design.sure"]),
    ("engineer", "D7", &["3c", "cost", "design.cost"]),
    ("engineer", "D8", &["D12"]),
    (
        "engineer",
        "D9",
        &["3d", "cross_project", "design.cross_project", "D13"],
    ),
    ("engineer", "D10", &["3e", "split", "design.split", "D14"]),
    (
        "engineer",
        "D11",
        &["3f", "adversarial_needed", "design.adversarial_needed", "D15"],
    ),
    ("engineer", "I1", &["4", "implement"]),
    ("engineer", "T1", &["6", "adversarial", "test.adversarial"]),
    ("engineer", "T2", &["7", "full_ci", "test.full_ci"]),
    ("engineer", "T3", &["5", "blind_review", "test.blind_review"]),
    ("engineer", "G3", &[]),
    ("engineer", "T4", &["5a", "table_review", "test.table_review"]),
    ("engineer", "T5", &["5b", "tree_review", "test.tree_review"]),
    ("engineer", "T6", &["8", "fix", "test.fix"]),
    ("engineer", "T7", &["18", "next_step", "test.next_step"]),
    ("engineer", "M1", &["19", "merge_gate", "merge.gate"]),
    ("engineer", "M2", &["9", "merge_queue", "merge.queue"]),
    ("engineer", "L1", &["10", "watch_main"]),
    ("engineer", "L2", &[]),
    ("engineer", "W1", &[]),
    ("engineer", "W2", &[]),
    ("engineer", "A4", &["11", "way_on", "ask.way_on"]),
    ("engineer", "A5", &["11a", "back", "ask.back"]),
    ("engineer", "A6", &["11b", "wait_items", "ask.wait_items"]),
    ("engineer", "A1", &["12", "advisor", "ask.advisor"]),
    ("engineer", "A2", &["15", "to_architect", "ask.architect"]),
    ("engineer", "A3", &["16", "to_owner", "ask.owner"]),
    ("engineer", "X1", &["13", "tools"]),
    ("engineer", "X5", &[]),
    ("engineer", "E1", &["20", "end"]),
    ("architect", "S0", &["W0", "wake", "wake.start"]),
    ("architect", "S1", &["W1", "understood", "wake.understood"]),
    ("architect", "U1", &["state", "understand.state"]),
    ("architect", "P1", &["U2", "policy_right", "understand.policy"]),
    ("architect", "N1", &["U3", "fit", "understand.fit"]),
    ("architect", "Q0", &["D0", "anything", "pick.anything"]),
    ("architect", "Q1", &["D1", "pick", "pick.choose"]),
    ("architect", "Q2", &["RT", "route", "pick.route"]),
    ("architect", "H1", &["T1", "set_straight", "handle.set_straight"]),
    ("architect", "H2", &["Q1", "crux", "handle.crux"]),
    ("architect", "H3", &["Q2", "ways", "handle.ways"]),
    ("architect", "H4", &["I1", "what_is_it", "handle.what_is_it"]),
    ("architect", "H5", &["I2", "take_in", "handle.take_in"]),
    ("architect", "H6", &["K1", "card_sound", "handle.card_sound"]),
    ("architect", "H7", &["F1", "have_fixed", "handle.have_fixed"]),
    ("architect", "H8", &["O1", "owner_words", "handle.owner_words"]),
    (
        "architect",
        "H9",
        &["M1", "answer_architect", "handle.answer_architect"],
    ),
    ("architect", "H10", &["N1", "what_runs", "handle.what_runs"]),
    ("architect", "H11", &[]),
    ("architect", "B1", &["batch_done", "handle.batch_done"]),
    ("architect", "B2", &["R1", "review", "batch.review"]),
    ("architect", "B3", &["X1", "revise", "batch.revise"]),
    ("architect", "B4", &["C1", "rethink", "batch.rethink"]),
    ("architect", "B5", &[]),
    ("architect", "E1", &["wrap_up"]),
    ("architect", "E2", &["record"]),
    ("architect", "P2", &["P1", "study", "policy.study"]),
    ("architect", "P3", &["P2", "owner_reads", "policy.owner_reads"]),
    ("architect", "P4", &["P3", "write", "policy.write"]),
    ("architect", "P5", &["P4", "assemble", "policy.assemble"]),
    ("architect", "P6", &["P5", "fresh_read", "policy.fresh_read"]),
    ("architect", "P7", &["P6", "judge_read", "policy.judge_read"]),
    ("architect", "P8", &["P7", "correct", "policy.correct"]),
    ("architect", "P9", &["P8", "count", "policy.count"]),
    ("architect", "P10", &["P11", "holds_up", "policy.holds_up"]),
    ("architect", "P11", &["P9", "owner_approves", "policy.owner_approves"]),
    ("architect", "P12", &["P10", "takes_effect", "policy.takes_effect"]),
    ("architect", "A3", &["ASK", "to_owner", "ask.owner"]),
    ("architect", "X1", &["TOOLS", "tools"]),
    ("architect", "X2", &["S1", "search"]),
    ("architect", "X5", &[]),
    ("architect", "L1", &[]),
    ("architect", "L2", &[]),
    ("architect", "G1", &[]),
    ("architect", "G2", &[]),
    ("architect", "H12", &[]),
];

/// Whether `id` is the code of a cell of `graph` now.
fn is_code(graph: &str, id: &str) -> bool {
    BEFORE.iter().any(|(g, c, _)| *g == graph && *c == id)
}

/// The ids the cell of `graph` coded `code` had before that are no cell's code now; none for a code the flow never
/// had another id for.
fn before(graph: &str, code: &str) -> Vec<&'static str> {
    BEFORE
        .iter()
        .filter(|(g, c, _)| *g == graph && *c == code)
        .flat_map(|x| x.2.iter().copied())
        .filter(|id| !is_code(graph, id))
        .collect()
}

/// Whether `id`, as a work or a round of any flow version has it, is the cell of `graph` coded `code`.
pub fn is_cell(graph: &str, code: &str, id: &str) -> bool {
    id == code || before(graph, code).contains(&id)
}

/// The code of the cell of `graph` that a work or a round of any flow version names `id`: `id` itself when it is a code
/// now, else the code of the cell it was an id of; none for an id no version gave a cell of the graph.
pub fn code_of(graph: &str, id: &str) -> Option<&'static str> {
    BEFORE
        .iter()
        .find(|(g, c, _)| *g == graph && *c == id)
        .or_else(|| {
            BEFORE
                .iter()
                .find(|(g, _, ids)| *g == graph && ids.contains(&id) && !is_code(graph, id))
        })
        .map(|x| x.1)
}

/// The cells of `graph` coded `codes`, as an SQL list of every id works and rounds of any flow version give them:
/// `('M2', '9', 'merge_queue', 'merge.queue')`.
pub fn cells_sql(graph: &str, codes: &[&str]) -> String {
    let ids: Vec<String> = codes
        .iter()
        .flat_map(|code| std::iter::once(*code).chain(before(graph, code)))
        .map(|id| format!("'{id}'"))
        .collect();
    format!("({})", ids.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A cell is named by its code and by every id it had before, in its own graph only, but for an id that is another
    // cell's code now.
    #[test]
    fn a_cell_is_named_as_every_version_names_it() {
        assert_eq!(
            cells_sql("engineer", &["M2"]),
            "('M2', '9', 'merge_queue', 'merge.queue')"
        );
        assert_eq!(cells_sql("engineer", &["I1"]), "('I1', '4', 'implement')");
        assert_eq!(
            cells_sql("architect", &["X1", "X2"]),
            "('X1', 'TOOLS', 'tools', 'X2', 'search')"
        );
        assert!(is_cell("engineer", "A3", "16") && is_cell("engineer", "A3", "to_owner"));
        assert!(!is_cell("architect", "A3", "16") && is_cell("architect", "A3", "ASK"));
        assert_eq!(code_of("engineer", "implement"), Some("I1"));
        assert_eq!(code_of("engineer", "I1"), Some("I1"));
        assert_eq!(code_of("architect", "T1"), Some("H1"));
        assert_eq!(code_of("architect", "P1"), Some("P1"));
        assert_eq!(code_of("engineer", "nowhere"), None);
        assert!(is_cell("architect", "A3", "ask.owner") && !is_cell("architect", "A3", "owner"));
        assert!(is_cell("architect", "P12", "takes_effect") && !is_cell("architect", "P12", "P10"));
        assert!(is_cell("architect", "H2", "handle.crux") && !is_cell("architect", "H2", "Q1"));
        assert!(is_cell("architect", "S0", "wake.start") && is_cell("architect", "B1", "handle.batch_done"));
        // The architect's start was W0 and W1: a round there reads as S0 and S1 now, and one of an earlier version at
        // the search's first code, S1, as the cell coded S1 now.
        assert_eq!(code_of("architect", "W0"), Some("S0"));
        assert_eq!(code_of("architect", "W1"), Some("S1"));
        assert_eq!(code_of("architect", "S1"), Some("S1"));
        assert_eq!(
            cells_sql("architect", &["S1"]),
            "('S1', 'W1', 'understood', 'wake.understood')"
        );
        // The start is S1 now, and S0 the cell an item waits at to start, which was U0: a round of an earlier version
        // at the start named S0 reads as the cell coded S0 now.
        assert_eq!(code_of("engineer", "start"), Some("S1"));
        assert_eq!(code_of("engineer", "U0"), Some("S0"));
        assert_eq!(code_of("engineer", "S0"), Some("S0"));
        assert_eq!(cells_sql("engineer", &["S1"]), "('S1', '0', 'start')");
        // The design stage numbered in the order it runs: an id that is another cell's code now reads as that cell, and
        // a cell's names before are its own.
        assert_eq!(code_of("engineer", "D4"), Some("D4"));
        assert_eq!(code_of("engineer", "choose"), Some("D2"));
        assert_eq!(code_of("engineer", "adversarial_needed"), Some("D11"));
        assert_eq!(
            cells_sql("engineer", &["D11"]),
            "('D11', '3f', 'adversarial_needed', 'design.adversarial_needed', 'D15')"
        );
        assert_eq!(
            cells_sql("engineer", &["D2"]),
            "('D2', '2a', 'choose', 'design.choose')"
        );
        // The stage numbered again when the tree left: the codes past its new end read as the cells they became.
        assert_eq!(code_of("engineer", "D12"), Some("D8"));
        assert_eq!(code_of("engineer", "D13"), Some("D9"));
        assert_eq!(code_of("engineer", "D14"), Some("D10"));
        assert_eq!(code_of("engineer", "D15"), Some("D11"));
        assert_eq!(code_of("engineer", "T5"), Some("T5"));
        assert_eq!(code_of("engineer", "G1"), Some("G1"));
        assert_eq!(code_of("architect", "X5"), Some("X5"));
        assert_eq!(code_of("architect", "G2"), Some("G2"));
        assert_eq!(cells_sql("architect", &["L1", "G1"]), "('L1', 'G1')");
    }
}
