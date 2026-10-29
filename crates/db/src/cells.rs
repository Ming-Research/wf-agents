//! The cells of the flow as works and rounds name them. A work and its rounds keep the cells' ids of the flow version
//! the work ran on. Cells had numbers (the engineer's graph) and codes (the architect's) first, then names, and now
//! names with their stage, as `test.fix`; the works and rounds of the versions before keep the ids they had. A reader of
//! the works and rounds of every version names a cell by every id it has had.

/// Each cell, by its graph and its name, with the ids it had in the flow versions before: its number or code, and the
/// name it had before names took their stage, where that was another.
const BEFORE: [(&str, &str, &[&str]); 69] = [
    ("engineer", "start", &["0"]),
    ("engineer", "research.problem", &["1", "research"]),
    ("engineer", "research.stands", &["1a", "stands"]),
    ("engineer", "design.compare", &["2", "compare"]),
    ("engineer", "design.choose", &["2a", "choose"]),
    ("engineer", "design.rethink", &["2b", "rethink_design"]),
    ("engineer", "design.gap_under_test", &["3a", "gap_under_test"]),
    ("engineer", "design.sure", &["3b", "sure"]),
    ("engineer", "design.cost", &["3c", "cost"]),
    ("engineer", "design.cross_project", &["3d", "cross_project"]),
    ("engineer", "design.split", &["3e", "split"]),
    ("engineer", "design.adversarial_needed", &["3f", "adversarial_needed"]),
    ("engineer", "implement", &["4"]),
    ("engineer", "test.adversarial", &["6", "adversarial"]),
    ("engineer", "test.full_ci", &["7", "full_ci"]),
    ("engineer", "test.blind_review", &["5", "blind_review"]),
    ("engineer", "test.table_review", &["5a", "table_review"]),
    ("engineer", "test.tree_review", &["5b", "tree_review"]),
    ("engineer", "test.fix", &["8", "fix"]),
    ("engineer", "test.next_step", &["18", "next_step"]),
    ("engineer", "merge.gate", &["19", "merge_gate"]),
    ("engineer", "merge.queue", &["9", "merge_queue"]),
    ("engineer", "watch_main", &["10"]),
    ("engineer", "ask.way_on", &["11", "way_on"]),
    ("engineer", "ask.back", &["11a", "back"]),
    ("engineer", "ask.wait_items", &["11b", "wait_items"]),
    ("engineer", "ask.advisor", &["12", "advisor"]),
    ("engineer", "ask.architect", &["15", "to_architect"]),
    ("engineer", "ask.owner", &["16", "to_owner"]),
    ("engineer", "tools", &["13"]),
    ("engineer", "end", &["20"]),
    ("architect", "wake.start", &["W0", "wake"]),
    ("architect", "wake.understood", &["W1", "understood"]),
    ("architect", "understand.state", &["U1", "state"]),
    ("architect", "understand.policy", &["U2", "policy_right"]),
    ("architect", "understand.fit", &["U3", "fit"]),
    ("architect", "pick.anything", &["D0", "anything"]),
    ("architect", "pick.choose", &["D1", "pick"]),
    ("architect", "pick.route", &["RT", "route"]),
    ("architect", "handle.set_straight", &["T1", "set_straight"]),
    ("architect", "handle.crux", &["Q1", "crux"]),
    ("architect", "handle.ways", &["Q2", "ways"]),
    ("architect", "handle.what_is_it", &["I1", "what_is_it"]),
    ("architect", "handle.take_in", &["I2", "take_in"]),
    ("architect", "handle.card_sound", &["K1", "card_sound"]),
    ("architect", "handle.have_fixed", &["F1", "have_fixed"]),
    ("architect", "handle.owner_words", &["O1", "owner_words"]),
    ("architect", "handle.answer_architect", &["M1", "answer_architect"]),
    ("architect", "handle.what_runs", &["N1", "what_runs"]),
    ("architect", "handle.batch_done", &["B1", "batch_done"]),
    ("architect", "batch.review", &["R1", "review"]),
    ("architect", "batch.revise", &["X1", "revise"]),
    ("architect", "batch.rethink", &["C1", "rethink"]),
    ("architect", "wrap_up", &["E1"]),
    ("architect", "record", &["E2"]),
    ("architect", "policy.study", &["P1", "study"]),
    ("architect", "policy.owner_reads", &["P2", "owner_reads"]),
    ("architect", "policy.write", &["P3", "write"]),
    ("architect", "policy.assemble", &["P4", "assemble"]),
    ("architect", "policy.fresh_read", &["P5", "fresh_read"]),
    ("architect", "policy.judge_read", &["P6", "judge_read"]),
    ("architect", "policy.correct", &["P7", "correct"]),
    ("architect", "policy.count", &["P8", "count"]),
    ("architect", "policy.holds_up", &["P11", "holds_up"]),
    ("architect", "policy.owner_approves", &["P9", "owner_approves"]),
    ("architect", "policy.takes_effect", &["P10", "takes_effect"]),
    ("architect", "ask.owner", &["ASK", "to_owner"]),
    ("architect", "tools", &["TOOLS"]),
    ("architect", "search", &["S1"]),
];

/// The ids a cell of `graph` named `name` had before; none for a name the flow never had another for.
fn before(graph: &str, name: &str) -> &'static [&'static str] {
    BEFORE
        .iter()
        .find(|(g, n, _)| *g == graph && *n == name)
        .map(|x| x.2)
        .unwrap_or_default()
}

/// Whether `id`, as a work or a round of any flow version has it, is the cell of `graph` named `name`.
pub fn is_cell(graph: &str, name: &str, id: &str) -> bool {
    id == name || before(graph, name).contains(&id)
}

/// The cells of `graph` named `names`, as an SQL list of every id works and rounds of any flow version give them:
/// `('merge.queue', '9', 'merge_queue')`.
pub fn cells_sql(graph: &str, names: &[&str]) -> String {
    let ids: Vec<String> = names
        .iter()
        .flat_map(|name| std::iter::once(*name).chain(before(graph, name).iter().copied()))
        .map(|id| format!("'{id}'"))
        .collect();
    format!("({})", ids.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A cell is named by its name and by every id it had before, in its own graph only.
    #[test]
    fn a_cell_is_named_as_every_version_names_it() {
        assert_eq!(
            cells_sql("engineer", &["merge.queue"]),
            "('merge.queue', '9', 'merge_queue')"
        );
        assert_eq!(cells_sql("engineer", &["implement"]), "('implement', '4')");
        assert_eq!(
            cells_sql("architect", &["tools", "search"]),
            "('tools', 'TOOLS', 'search', 'S1')"
        );
        assert!(is_cell("engineer", "ask.owner", "16") && is_cell("engineer", "ask.owner", "to_owner"));
        assert!(!is_cell("architect", "ask.owner", "16") && is_cell("architect", "ask.owner", "ASK"));
        assert!(is_cell("architect", "ask.owner", "ask.owner") && !is_cell("architect", "ask.owner", "owner"));
    }
}
