//! The script the test carriers follow: a TOML file saying, for each flow, what each cell's agent or act returns, run
//! by run and round by round, and what each tool answers.
//!
//! ```toml
//! # How many rounds of agents go on at once; two where it is left out, as the installation's sample settings give.
//! slots = 3
//!
//! # What a round takes where its entry says nothing; each is optional.
//! [took]
//! ms = 1500
//! usd_micros = 20000
//! input_tokens = 3000
//! cached_tokens = 12000
//! output_tokens = 400
//!
//! # What a tool answers, and how long it takes.
//! [tools.ci]
//! answer = "Every check passed."
//! ms = 2000
//!
//! # The value a round hands in for a choice where it names none; the choice's first value otherwise.
//! [flows.trial.values]
//! verdict = "yes"
//!
//! # The second round of the first run of the cell review hands in these blocks.
//! [[flows.trial.rounds]]
//! cell = "review"
//! run = 1
//! round = 2
//! took = { ms = 4000 }
//! hands_in = [
//!     { block = "opinions", content = "Name the error.", matters = true },
//!     { block = "verdict", value = "no" },
//! ]
//!
//! # The first round of the first run of plan asks the owner a question, and a tool to run.
//! [[flows.trial.rounds]]
//! cell = "plan"
//! asks = [{ question = "Which name?", who = "owner" }, { tool = "ci", input = "the branch" }]
//!
//! # The first round of the second run of plan fails: nothing comes back, and the work is paused with this reason.
//! [[flows.trial.rounds]]
//! cell = "plan"
//! run = 2
//! fails = "The command line program could not start."
//!
//! # The second round of that run reaches the account's usage limit, which resets at this time: no agent's round of
//! # any work begins until then.
//! [[flows.trial.rounds]]
//! cell = "plan"
//! run = 2
//! round = 2
//! fails = "You've hit your limit."
//! failure = "usage_limit"
//! resets = 1792962000000
//! ```
//!
//! A round that fails takes what its entry says, as any round does. Once the owner lets its work on, the run's next
//! round is given the same, and does what the script says of that round: a script makes a round fail again by saying
//! so of the next round too.
//!
//! A run is the cell's nth run in its work, and a round the nth of its run, each counted from one; `run` and `round`
//! are one where they are left out. Where the script says nothing of a round, the round hands in each output of its
//! cell that no cell before it reads, and each the cell must write, each with a short content made up for it, saying
//! the change matters, and with the value the script names for the block or else its first, where the block is a
//! choice. An output a cell before it reads is a way back, an opinion on what that cell did, which an agent writes only
//! when it has one: by default it has none, so that a script that says nothing of a judging cell has it content and
//! does not send the work round until its limit; one the cell must write each time it writes all the same, so that the
//! round is not sent back, and, where the block had a version as the run started, says the change does not matter, so
//! that the work is not sent round for it either. A block handed in with no content or no value is given them the same
//! way. The engine's own acts take no cost and no tokens. What a round returns depends on nothing but its flow, cell,
//! run and round, and the versions its run started from, which its start recorded, so that a round given again after
//! the process started again returns what it would have.

use std::collections::BTreeMap;

use flow::{CellSpec, Compiled, Kind};
use serde::Deserialize;
use work::{Ask, Handed, Spent, Time};

use decide::run::Failure;
use decide::{BlockId, Seq, Who};
use engine::Came;

/// How long a round takes where the script says nothing, in milliseconds.
pub const ROUND_MS: u64 = 200;
/// What an agent's round costs where the script says nothing, in millionths of a dollar.
pub const USD_MICROS: u64 = 2_000;
/// The tokens an agent's round sends that are not read from the cache, where the script says nothing.
pub const INPUT_TOKENS: u64 = 1_000;
/// The tokens an agent's round sends that are read from the cache, where the script says nothing.
pub const CACHED_TOKENS: u64 = 0;
/// The tokens that come back from an agent's round, where the script says nothing.
pub const OUTPUT_TOKENS: u64 = 100;
/// How long a tool takes where the script says nothing, in milliseconds.
pub const TOOL_MS: u64 = 100;
/// How many rounds of agents go on at once where the script says nothing: as many as the installation's sample
/// settings give.
pub const SLOTS: usize = 2;

/// A script, as its file is written.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// How many rounds of agents go on at once; [`SLOTS`] where the script says none.
    #[serde(default)]
    pub slots: Option<usize>,
    /// What a round takes where its entry says nothing.
    #[serde(default)]
    pub took: Took,
    /// What each tool answers, by its name.
    #[serde(default)]
    pub tools: BTreeMap<String, ToolScript>,
    /// What the cells of each flow return, by the flow's name.
    #[serde(default)]
    pub flows: BTreeMap<String, FlowScript>,
}

/// What a round takes; what is left out is taken from the script's own, or else from the defaults above.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Took {
    /// How long it is under way, in milliseconds.
    pub ms: Option<u64>,
    pub usd_micros: Option<u64>,
    pub input_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// What a tool answers.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolScript {
    /// Its result; with none, one that says what it ran on.
    pub answer: Option<String>,
    /// How long it takes, in milliseconds.
    pub ms: Option<u64>,
}

/// What the cells of one flow return.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowScript {
    /// The value handed in for a choice where a round names none, by the block.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub rounds: Vec<RoundScript>,
}

/// What one round returns.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundScript {
    pub cell: String,
    /// The cell's nth run in its work.
    #[serde(default = "first")]
    pub run: usize,
    /// The nth round of the run.
    #[serde(default = "first")]
    pub round: usize,
    #[serde(default)]
    pub took: Took,
    /// The blocks it hands in.
    pub hands_in: Option<Vec<HandIn>>,
    /// The calls it asks for.
    pub asks: Option<Vec<AskScript>>,
    /// Why it fails, where it fails: nothing comes back.
    pub fails: Option<String>,
    /// The kind of cause of its failure, as a carrier tells it apart (`usage_limit`, `passing`, ...); a cause no
    /// kind tells apart, which pauses the work, where the script names none.
    pub failure: Option<Failure>,
    /// When the usage limit its failure reached resets, in milliseconds since the start of 1970.
    pub resets: Option<u64>,
}

fn first() -> usize {
    1
}

fn yes() -> bool {
    true
}

/// A block a round hands in.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandIn {
    pub block: String,
    pub content: Option<String>,
    pub value: Option<String>,
    /// Whether the round says the change matters to the cells that read the block.
    #[serde(default = "yes")]
    pub matters: bool,
}

/// A call a round asks for.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum AskScript {
    Tool(ToolAsk),
    Question(QuestionAsk),
}

/// A tool to run.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolAsk {
    pub tool: String,
    /// What it is run on.
    #[serde(default)]
    pub input: String,
}

/// A question for a person.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionAsk {
    pub question: String,
    #[serde(default)]
    pub who: Person,
}

/// Who a question is for.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Person {
    #[default]
    Owner,
    Architect,
}

/// What a round returns, and what it takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub ends: Ends,
    pub spent: Spent,
    /// How long it is under way, in milliseconds.
    pub ms: u64,
}

/// How a round ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ends {
    /// This comes back.
    Came(Came),
    /// It fails, for this reason, of this kind of cause: nothing comes back.
    Fails {
        why: String,
        failure: Failure,
        resets: Option<Time>,
    },
}

impl Script {
    /// Reads a script from the text of its file, or says why it cannot be read.
    pub fn read(text: &str) -> Result<Script, String> {
        toml::from_str(text).map_err(|e| e.to_string().trim_end().to_string())
    }

    /// Everything wrong with the script for these flows, each in words with where it is: a flow, a cell or a block
    /// it names that is not there, a default value that is not one of its choice's, a round that does more than one of
    /// handing in, asking and failing, a round said twice, and an act of the engine that asks or costs. A script that names a block a cell may
    /// not write, or a value a choice does not have, is not wrong: what it hands in is sent back.
    pub fn check(&self, flows: &[Compiled]) -> Vec<String> {
        let mut faults = Vec::new();
        if self.slots == Some(0) {
            faults.push("slots: at least one round must be able to go on".to_string());
        }
        for (name, script) in &self.flows {
            let Some(flow) = flows.iter().find(|flow| flow.name == *name) else {
                faults.push(format!("flows.{name}: no flow is named {name}"));
                continue;
            };
            for (block, value) in &script.values {
                let at = format!("flows.{name}.values.{block}");
                match flow.blocks.get(&BlockId::from(block.as_str())).map(|b| &b.values) {
                    None => faults.push(format!("{at}: the flow {name} has no block {block}")),
                    Some(None) => faults.push(format!("{at}: the block {block} is no choice")),
                    Some(Some(values)) if !values.contains(value) => {
                        faults.push(format!("{at}: {value} is not one of the values of {block}"));
                    }
                    Some(Some(_)) => {}
                }
            }
            for (i, round) in script.rounds.iter().enumerate() {
                let at = format!("flows.{name}.rounds[{i}]");
                let said = (script.rounds[..i].iter())
                    .position(|r| r.cell == round.cell && r.run == round.run && r.round == round.round);
                if let Some(k) = said {
                    faults.push(format!("{at}: says again what flows.{name}.rounds[{k}] says"));
                }
                if round.run == 0 || round.round == 0 {
                    faults.push(format!("{at}: runs and rounds are counted from 1"));
                }
                if round.hands_in.is_some() && round.asks.is_some() {
                    faults.push(format!("{at}: a round either hands in or asks, not both"));
                }
                if round.fails.is_some() && (round.hands_in.is_some() || round.asks.is_some()) {
                    faults.push(format!("{at}: a round that fails neither hands in nor asks"));
                }
                if round.fails.is_none() && (round.failure.is_some() || round.resets.is_some()) {
                    faults.push(format!(
                        "{at}: only a round that fails has a kind of failure or a reset"
                    ));
                }
                for (j, handed) in round.hands_in.iter().flatten().enumerate() {
                    if !flow.blocks.contains_key(&BlockId::from(handed.block.as_str())) {
                        let block = &handed.block;
                        faults.push(format!("{at}.hands_in[{j}]: the flow {name} has no block {block}"));
                    }
                }
                match flow.cells.iter().find(|cell| cell.id.as_str() == round.cell) {
                    None => faults.push(format!("{at}: the flow {name} has no cell {}", round.cell)),
                    Some(cell) if cell.kind == Kind::Engine => {
                        if round.asks.is_some() {
                            faults.push(format!("{at}: an act of the engine asks for nothing"));
                        }
                        let took = round.took;
                        let costs = [
                            took.usd_micros,
                            took.input_tokens,
                            took.cached_tokens,
                            took.output_tokens,
                        ];
                        if costs.iter().any(Option::is_some) {
                            faults.push(format!("{at}: an act of the engine takes no cost and no tokens"));
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        faults
    }

    /// What the round of this number of the cell's nth run returns in the flow, and what it takes; `basis` is the
    /// version of each block the run started from, as its start was recorded.
    pub fn round(
        &self,
        flow: &Compiled,
        cell: &CellSpec,
        run: usize,
        number: usize,
        basis: &BTreeMap<BlockId, Option<Seq>>,
    ) -> Planned {
        let script = self.flows.get(&flow.name);
        let entry = (script.into_iter())
            .flat_map(|script| &script.rounds)
            .find(|r| r.cell == cell.id.as_str() && r.run == run && r.round == number);
        let took = entry.map(|entry| entry.took).unwrap_or_default();
        let or = |round: Option<u64>, script: Option<u64>, default: u64| round.or(script).unwrap_or(default);
        let spent = match cell.kind {
            Kind::Engine => Spent::default(),
            Kind::Agent => Spent {
                usd_micros: or(took.usd_micros, self.took.usd_micros, USD_MICROS),
                input_tokens: or(took.input_tokens, self.took.input_tokens, INPUT_TOKENS),
                cached_tokens: or(took.cached_tokens, self.took.cached_tokens, CACHED_TOKENS),
                output_tokens: or(took.output_tokens, self.took.output_tokens, OUTPUT_TOKENS),
                ..Default::default()
            },
        };
        let content = |block: &str| format!("The {block} as {} wrote it in its run {run}, round {number}.", cell.id);
        let value = |block: &str| {
            let named = script.and_then(|script| script.values.get(block)).cloned();
            let choice = flow.blocks.get(&BlockId::from(block)).and_then(|b| b.values.as_ref());
            choice.and_then(|values| named.or_else(|| values.first().cloned()))
        };
        let came = match entry {
            Some(RoundScript {
                fails: Some(why),
                failure,
                resets,
                ..
            }) => {
                return Planned {
                    ends: Ends::Fails {
                        why: why.clone(),
                        failure: failure.unwrap_or_default(),
                        resets: resets.map(Time),
                    },
                    spent,
                    ms: or(took.ms, self.took.ms, ROUND_MS),
                };
            }
            Some(RoundScript { asks: Some(asks), .. }) => Came::asks(asks.iter().map(AskScript::ask).collect()),
            Some(RoundScript {
                hands_in: Some(handed), ..
            }) => Came::hands_in(
                (handed.iter())
                    .map(|h| Handed {
                        block: BlockId(h.block.clone()),
                        content: h.content.clone().unwrap_or_else(|| content(&h.block)),
                        matters: h.matters,
                        value: h.value.clone().or_else(|| value(&h.block)),
                    })
                    .collect(),
            ),
            _ => {
                let place = flow.flow.place(&cell.id);
                let outputs = place.map_or(&[][..], |place| &flow.flow.cells()[place].outputs);
                // An output an earlier cell reads is a way back, an opinion on what that cell did: an agent writes one
                // only when it has one, and by default it has none, unless the cell must write it each time; then,
                // once the block has a version, it is written as a change that does not matter.
                let back = |block: &BlockId| {
                    let readers = flow
                        .blocks
                        .get(block)
                        .map(|b| b.readers.cells.iter().chain(&b.readers.conditions));
                    (readers.into_iter().flatten())
                        .any(|reader| matches!((flow.flow.place(reader), place), (Some(r), Some(p)) if r < p))
                };
                Came::hands_in(
                    (outputs.iter())
                        .filter(|block| !back(block) || cell.must_write.contains(block))
                        .map(|block| Handed {
                            block: block.clone(),
                            content: content(block.as_str()),
                            matters: !back(block) || !basis.get(block).is_some_and(Option::is_some),
                            value: value(block.as_str()),
                        })
                        .collect(),
                )
            }
        };
        Planned {
            ends: Ends::Came(came),
            spent,
            ms: or(took.ms, self.took.ms, ROUND_MS),
        }
    }

    /// What the tool answers when run on this, and how long it takes, in milliseconds.
    pub fn tool(&self, name: &str, input: &str) -> (String, u64) {
        let script = self.tools.get(name);
        let answer = (script.and_then(|s| s.answer.clone())).unwrap_or_else(|| format!("{name} ran on: {input}"));
        (answer, script.and_then(|s| s.ms).unwrap_or(TOOL_MS))
    }
}

impl AskScript {
    fn ask(&self) -> Ask {
        match self {
            AskScript::Tool(tool) => Ask::Tool {
                name: tool.tool.clone(),
                input: tool.input.clone(),
            },
            AskScript::Question(question) => Ask::Person {
                who: match question.who {
                    Person::Owner => Who::Owner,
                    Person::Architect => Who::Architect,
                },
                question: question.question.clone(),
            },
        }
    }
}
