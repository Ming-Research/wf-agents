//! The page's TypeScript types, generated from the Rust types: the interface's own and the compiled flow's.
//! `client/src/api.ts` holds what this gives; the api crate's test `typescript` fails when it differs.

use ts_rs::{Config, TS};

use crate::*;

const HEADER: &str =
    "// The types of the owner's interface, generated from the Rust types in crates/api and crates/flow. Do not edit:
// change the Rust types and run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` to write this file again.
";

fn decl<T: TS + ?Sized + 'static>(cfg: &Config) -> String {
    format!("{}export {}", T::docs().unwrap_or_default(), T::decl(cfg))
}

/// The whole of `client/src/api.ts`.
pub fn typescript() -> String {
    // Integers are ids, counts and times in milliseconds, all well inside what a JavaScript number holds exactly.
    let cfg = Config::new().with_large_int("number");
    let decls = [
        decl::<serde_json::Value>(&cfg),
        // The interface.
        decl::<Mode>(&cfg),
        decl::<StepTaken>(&cfg),
        decl::<Status>(&cfg),
        decl::<Halt>(&cfg),
        decl::<Event>(&cfg),
        decl::<WorkState>(&cfg),
        decl::<RunningRound>(&cfg),
        decl::<WorkSummary>(&cfg),
        decl::<RoundOutcome>(&cfg),
        decl::<RoundSummary>(&cfg),
        decl::<Card>(&cfg),
        decl::<Interjection>(&cfg),
        decl::<WorkDetail>(&cfg),
        decl::<Session>(&cfg),
        decl::<RoundInput>(&cfg),
        decl::<RoundMaterial>(&cfg),
        decl::<KeptFile>(&cfg),
        decl::<ToolRun>(&cfg),
        decl::<RoundDetail>(&cfg),
        decl::<FlowVersion>(&cfg),
        decl::<ItemState>(&cfg),
        decl::<ItemSummary>(&cfg),
        decl::<ItemLink>(&cfg),
        decl::<Items>(&cfg),
        decl::<LinkedItem>(&cfg),
        decl::<ItemDetail>(&cfg),
        decl::<Shown>(&cfg),
        decl::<SetMode>(&cfg),
        decl::<ApiError>(&cfg),
        // The flow page.
        decl::<FlowView>(&cfg),
        decl::<GraphView>(&cfg),
        decl::<CellView>(&cfg),
        decl::<RowRef>(&cfg),
        decl::<Origin>(&cfg),
        decl::<Material>(&cfg),
        decl::<MaterialOf>(&cfg),
        decl::<Returns>(&cfg),
        decl::<ReturnForm>(&cfg),
        decl::<FieldKind>(&cfg),
        decl::<Field>(&cfg),
        decl::<Branch>(&cfg),
        decl::<Use>(&cfg),
        decl::<FlowDiff>(&cfg),
        decl::<GraphDiff>(&cfg),
        decl::<ChangeKind>(&cfg),
        decl::<CellDiff>(&cfg),
        decl::<Part>(&cfg),
        decl::<Change>(&cfg),
        decl::<FlowSource>(&cfg),
        decl::<FlowAt>(&cfg),
        // The compiled flow.
        decl::<flow::Compiled>(&cfg),
        decl::<flow::format::EngineNames>(&cfg),
        decl::<flow::ToolUse>(&cfg),
        decl::<flow::CompiledGraph>(&cfg),
        decl::<flow::format::Counter>(&cfg),
        decl::<flow::format::Rule>(&cfg),
        decl::<flow::CompiledInterject>(&cfg),
        decl::<flow::format::Named>(&cfg),
        decl::<flow::format::Rounds>(&cfg),
        decl::<flow::format::SentBack>(&cfg),
        decl::<flow::format::Failed>(&cfg),
        decl::<flow::format::Failure>(&cfg),
        decl::<flow::format::Handle>(&cfg),
        decl::<flow::CompiledCell>(&cfg),
        decl::<flow::CompiledInput>(&cfg),
        decl::<flow::format::Src>(&cfg),
        decl::<flow::RuleUse>(&cfg),
        decl::<flow::format::PromptBy>(&cfg),
        decl::<flow::format::Form>(&cfg),
        decl::<flow::CompiledRow>(&cfg),
        decl::<flow::format::Atom>(&cfg),
        decl::<flow::format::Do>(&cfg),
        decl::<flow::format::DoAt>(&cfg),
        decl::<flow::format::Always>(&cfg),
        decl::<flow::format::Check>(&cfg),
    ];
    format!("{HEADER}\n{}\n", decls.join("\n\n"))
}
