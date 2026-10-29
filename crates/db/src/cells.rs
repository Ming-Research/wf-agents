//! The cells of the flow as works and rounds name them. A work and its rounds keep the cells' ids of the flow version
//! the work ran on; before cells had names they had numbers (the engineer's graph) and codes (the architect's), which
//! the works and rounds of those versions keep. A reader of the works and rounds of every version names a cell by both.

/// Each cell, by its graph and its name, with the id it had in the flow versions before cells had names.
const BEFORE: [(&str, &str, &str); 69] = [
    ("engineer", "start", "0"),
    ("engineer", "research", "1"),
    ("engineer", "stands", "1a"),
    ("engineer", "compare", "2"),
    ("engineer", "choose", "2a"),
    ("engineer", "rethink_design", "2b"),
    ("engineer", "gap_under_test", "3a"),
    ("engineer", "sure", "3b"),
    ("engineer", "cost", "3c"),
    ("engineer", "cross_project", "3d"),
    ("engineer", "split", "3e"),
    ("engineer", "adversarial_needed", "3f"),
    ("engineer", "implement", "4"),
    ("engineer", "adversarial", "6"),
    ("engineer", "full_ci", "7"),
    ("engineer", "blind_review", "5"),
    ("engineer", "table_review", "5a"),
    ("engineer", "tree_review", "5b"),
    ("engineer", "fix", "8"),
    ("engineer", "next_step", "18"),
    ("engineer", "merge_gate", "19"),
    ("engineer", "merge_queue", "9"),
    ("engineer", "watch_main", "10"),
    ("engineer", "way_on", "11"),
    ("engineer", "back", "11a"),
    ("engineer", "wait_items", "11b"),
    ("engineer", "advisor", "12"),
    ("engineer", "to_architect", "15"),
    ("engineer", "to_owner", "16"),
    ("engineer", "tools", "13"),
    ("engineer", "end", "20"),
    ("architect", "wake", "W0"),
    ("architect", "understood", "W1"),
    ("architect", "state", "U1"),
    ("architect", "policy_right", "U2"),
    ("architect", "fit", "U3"),
    ("architect", "anything", "D0"),
    ("architect", "pick", "D1"),
    ("architect", "route", "RT"),
    ("architect", "set_straight", "T1"),
    ("architect", "crux", "Q1"),
    ("architect", "ways", "Q2"),
    ("architect", "what_is_it", "I1"),
    ("architect", "take_in", "I2"),
    ("architect", "card_sound", "K1"),
    ("architect", "have_fixed", "F1"),
    ("architect", "owner_words", "O1"),
    ("architect", "answer_architect", "M1"),
    ("architect", "what_runs", "N1"),
    ("architect", "batch_done", "B1"),
    ("architect", "review", "R1"),
    ("architect", "revise", "X1"),
    ("architect", "rethink", "C1"),
    ("architect", "wrap_up", "E1"),
    ("architect", "record", "E2"),
    ("architect", "study", "P1"),
    ("architect", "owner_reads", "P2"),
    ("architect", "write", "P3"),
    ("architect", "assemble", "P4"),
    ("architect", "fresh_read", "P5"),
    ("architect", "judge_read", "P6"),
    ("architect", "correct", "P7"),
    ("architect", "count", "P8"),
    ("architect", "holds_up", "P11"),
    ("architect", "owner_approves", "P9"),
    ("architect", "takes_effect", "P10"),
    ("architect", "to_owner", "ASK"),
    ("architect", "tools", "TOOLS"),
    ("architect", "search", "S1"),
];

/// Whether `id`, as a work or a round of any flow version has it, is the cell of `graph` named `name`.
pub fn is_cell(graph: &str, name: &str, id: &str) -> bool {
    id == name || BEFORE.iter().any(|(g, n, b)| *g == graph && *n == name && *b == id)
}

/// The cells of `graph` named `names`, as an SQL list of every id works and rounds of any flow version give them:
/// `('merge_queue', '9')`.
pub fn cells_sql(graph: &str, names: &[&str]) -> String {
    let ids: Vec<String> = names
        .iter()
        .flat_map(|name| {
            let before = BEFORE
                .iter()
                .filter(move |(g, n, _)| *g == graph && n == name)
                .map(|x| x.2);
            std::iter::once(*name).chain(before)
        })
        .map(|id| format!("'{id}'"))
        .collect();
    format!("({})", ids.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A cell is named by its name and by the id it had before cells had names, in its own graph only.
    #[test]
    fn a_cell_is_named_as_every_version_names_it() {
        assert_eq!(cells_sql("engineer", &["merge_queue"]), "('merge_queue', '9')");
        assert_eq!(
            cells_sql("architect", &["tools", "search"]),
            "('tools', 'TOOLS', 'search', 'S1')"
        );
        assert!(is_cell("engineer", "to_owner", "16") && is_cell("engineer", "to_owner", "to_owner"));
        assert!(!is_cell("architect", "to_owner", "16") && is_cell("architect", "to_owner", "ASK"));
    }
}
