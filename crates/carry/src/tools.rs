//! The tools an agent may ask for: `query`, one read-only SQL query on the store, run in a thread of its own and
//! interrupted where it is withdrawn (the module `query`). A tool's result is all an agent learns of it, so what goes
//! wrong in running it is said in its result, once: it is not tried again here, and the agent may ask again. The result
//! is kept whole in the records, and a result too long for a message in a file of the temporary directory
//! ([`keep_wholes`], removed by [`sweep`]); what a message gives of it is cut as the tool's words say ([`given`]). So
//! is the result of the tests a cell asked for, which the act that runs them writes into a block of the cell's own, as
//! the act's words say ([`tested`]).

mod query;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::{BlockId, CallId, Seq};
use flow::Compiled;
use flow::agents::{ResultGiven, Tool, fill};
use service::{CallReply, ToolCall, Tools as ToolsTrait, WITHDRAWN};
use store::WorkNumber;

use crate::held::Held;

pub use query::{QUERY, Querying};

/// The tools.
pub struct Tools {
    inner: Arc<Inner>,
}

struct Inner {
    held: Held<(WorkNumber, CallId), CallReply>,
    /// The tool `query`, closed until the settings open it.
    query: Querying,
    /// What a call is answered that names the record its cell asks for tests in, `{name}` the record's name: in a work
    /// kept from before, a cell asked for tests by a tool, which it now asks for in the record.
    as_a_tool: String,
}

impl Tools {
    /// The tools, the tool `query` as this says it is, a call that names the record its cell asks for tests in answered
    /// so.
    pub fn new(query: Querying, as_a_tool: String) -> Self {
        Tools {
            inner: Arc::new(Inner {
                held: Held::new(),
                query,
                as_a_tool,
            }),
        }
    }
}

/// Whether the call names the record its run's cell asks for tests in: what it asked for is asked for by an entry of
/// it now.
fn names_a_record(call: &ToolCall) -> bool {
    let block = BlockId::from(call.name.as_str());
    let cell = (call.kept.records.started.iter())
        .find(|s| s.run == call.run)
        .map(|s| &s.cell);
    (cell.and_then(|cell| call.flow.flow.place(cell)))
        .is_some_and(|i| (call.flow.flow.cells()[i].asks.iter()).any(|asks| asks.record == block))
}

impl ToolsTrait for Tools {
    fn call(&self, call: ToolCall, reply: CallReply) {
        self.prune(&call);
        let key = (call.work, call.call);
        let saying = reply.saying();
        let Some(stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let mut stop = stop.saying(saying);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let result = if call.name == QUERY {
                // The query runs in a thread of its own, within its time limit; withdrawn, it is interrupted.
                let asked = inner.clone();
                let input = call.input.clone();
                let cancel = Arc::new(query::Cancel::default());
                let told = cancel.clone();
                let mut running = tokio::task::spawn_blocking(move || asked.query.answer(&input, &told));
                tokio::select! {
                    said = &mut running => said.unwrap_or_else(|_| inner.query.unavailable()),
                    () = stop.told() => {
                        cancel.tell();
                        WITHDRAWN.to_string()
                    }
                }
            } else if names_a_record(&call) {
                flow::agents::fill(&inner.as_a_tool, &[("name", &call.name)])
            } else {
                let tools = flow::form::tools(&call.flow).join(", ");
                format!("No such tool: {}. The tools are: {tools}.", call.name)
            };
            if let Some(reply) = inner.held.take(key) {
                reply.done(result);
            }
        });
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        self.prune(&call);
        if let Some(reply) = self.inner.held.stop((call.work, call.call), reply) {
            reply.done(WITHDRAWN.to_string());
        }
    }
}

impl Tools {
    /// Forgets the calls of the work whose result the loop has taken, as its records show them.
    fn prune(&self, call: &ToolCall) {
        let done = &call.kept.records.done;
        (self.inner.held).prune(|(work, id)| *work == call.work && done.iter().any(|d| d.call == *id));
    }
}

/// Each result of the work a message gives otherwise than the records hold it, the records holding each whole: a
/// result of the tool `query` longer than its file says a message gives, cut as the tool cuts it; one of a tool whose
/// file gives a bound, cut at it, with the words that follow a cut one; one of a tool no file defines now, as a call of
/// `run_tests` kept from before the cells asked for tests in a record, as the tests' results are (`unknown`); and any,
/// its end of whitespace left off. A cut one says which file of `dir` holds its whole ([`keep_wholes`]).
pub fn given(
    work: &work::Work,
    (tools, unknown): (&BTreeMap<String, Tool>, &ResultGiven),
    dir: &Path,
) -> BTreeMap<CallId, String> {
    (work.calls.iter())
        .filter_map(|(call, asked)| {
            let work::Call::Asked(work::Ask::Tool { name, input }) = asked else {
                return None;
            };
            let result = work.results.get(call)?;
            let tool = tools.get(name);
            let file = file_of(dir, *call).display().to_string();
            let given = match tool.and_then(|t| t.given_bytes) {
                Some(bytes) if name == QUERY => Querying::cut(result.trim_end(), input, bytes, &file),
                _ => cut(result, tool.map_or(Some(unknown), |t| t.given.as_ref()), &file),
            };
            Some((*call, given))
        })
        .collect()
}

/// Each version of a block the result of the tests a cell asked for comes in ([`decide::Flow::is_result`]), as a round
/// of the cell is given it: cut at the bound the act's words give, with the words that follow a cut one, which say
/// which file of `dir` holds its whole ([`keep_wholes`]); and any, its end of whitespace left off.
pub fn tested(work: &work::Work, flow: &Compiled, given: &ResultGiven, dir: &Path) -> BTreeMap<(BlockId, Seq), String> {
    (results_of_tests(work, flow))
        .map(|(block, at, result)| {
            let file = tested_file(dir, block, at).display().to_string();
            ((block.clone(), at), cut(result, Some(given), &file))
        })
        .collect()
}

/// Each version of a block of the flow that the result of the tests a cell asked for comes in, with its content.
fn results_of_tests<'w>(work: &'w work::Work, flow: &Compiled) -> impl Iterator<Item = (&'w BlockId, Seq, &'w str)> {
    (work.contents.iter())
        .filter(move |((block, _), _)| flow.flow.is_result(block))
        .map(|((block, at), content)| (block, *at, content.as_str()))
}

/// How many days a file of a cut result is kept after it was last written ([`sweep`]).
pub const KEPT_DAYS: u64 = 7;

/// Where the whole results of a work are kept for its agent to read: a directory of the temporary directory, which the
/// agents' command line is let read, and never of a worktree, so that nothing of it can be committed. The
/// installation's tag keeps two installations of a machine apart.
pub fn files_dir(temp: &Path, tag: &str, work: WorkNumber) -> PathBuf {
    temp.join(format!("wf-agents-{tag}-results")).join(work.0.to_string())
}

/// The file the whole result of a call is kept in.
fn file_of(dir: &Path, call: CallId) -> PathBuf {
    dir.join(format!("{}.txt", call.0))
}

/// The file the whole result of the tests a cell asked for is kept in, by the version of the block it came in.
fn tested_file(dir: &Path, block: &BlockId, at: Seq) -> PathBuf {
    dir.join(format!("{block}-{}.txt", at.0))
}

/// Whether a message gives this result of the tool cut.
fn is_cut(name: &str, result: &str, tool: Option<&Tool>) -> bool {
    let Some(tool) = tool else { return false };
    match (name == QUERY, tool.given_bytes, &tool.given) {
        (true, Some(bytes), _) => result.trim_end().len() > bytes,
        (_, _, Some(given)) => result.len() > given.bound,
        _ => false,
    }
}

/// The results a message gives cut, each with the file of `dir` that [`given`] or [`tested`] says holds its whole.
pub fn wholes<'a>(
    work: &'a work::Work,
    (tools, flow, tests): (&BTreeMap<String, Tool>, &Compiled, &ResultGiven),
    dir: &Path,
) -> Vec<(PathBuf, &'a str)> {
    let calls = (work.calls.iter()).filter_map(|(call, asked)| {
        let work::Call::Asked(work::Ask::Tool { name, .. }) = asked else {
            return None;
        };
        let result = work.results.get(call)?;
        let cut = match tools.get(name) {
            Some(tool) => is_cut(name, result, Some(tool)),
            None => result.len() > tests.bound,
        };
        cut.then(|| (file_of(dir, *call), result.as_str()))
    });
    let tested = (results_of_tests(work, flow))
        .filter(|(_, _, result)| result.len() > tests.bound)
        .map(|(block, at, result)| (tested_file(dir, block, at), result));
    calls.chain(tested).collect()
}

/// Writes each whole result [`wholes`] names, each time a round is made ready, so that the files of a work going on are
/// as young as its last round.
pub async fn keep_wholes(
    work: &work::Work,
    of: (&BTreeMap<String, Tool>, &Compiled, &ResultGiven),
    dir: &Path,
) -> Result<(), String> {
    let wholes = wholes(work, of, dir);
    if wholes.is_empty() {
        return Ok(());
    }
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    for (file, result) in wholes {
        tokio::fs::write(&file, result)
            .await
            .map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(())
}

/// Removes, from `root` (the directory [`files_dir`] puts each work's directory in), each file written more than
/// [`KEPT_DAYS`] days before `now`, and each work's directory left empty. What cannot be removed is left.
pub async fn sweep(root: &Path, now: std::time::SystemTime) {
    let old = std::time::Duration::from_secs(KEPT_DAYS * 24 * 3600);
    let Ok(mut works) = tokio::fs::read_dir(root).await else {
        return;
    };
    while let Ok(Some(work)) = works.next_entry().await {
        let Ok(mut files) = tokio::fs::read_dir(work.path()).await else {
            continue;
        };
        while let Ok(Some(file)) = files.next_entry().await {
            let written = file.metadata().await.and_then(|m| m.modified());
            if written.is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age > old)) {
                let _ = tokio::fs::remove_file(file.path()).await;
            }
        }
        // Fails where a file is left, which is what keeps it.
        let _ = tokio::fs::remove_dir(work.path()).await;
    }
}

/// A result as it is given: its end of whitespace left off, and cut at the most bytes the words allow, at a whole
/// character, with the words that follow a cut one saying how much was given, how many bytes the record holds, and the
/// `file` that holds it whole; as it is where they allow it all.
fn cut(result: &str, given: Option<&ResultGiven>, file: &str) -> String {
    let Some(given) = given.filter(|g| result.len() > g.bound) else {
        return result.trim_end().to_string();
    };
    let mut at = given.bound;
    while !result.is_char_boundary(at) {
        at -= 1;
    }
    let (kept, size) = (at.to_string(), result.len().to_string());
    let words = fill(&given.cut, &[("given", &kept), ("size", &size), ("file", file)]);
    format!("{}\n\n{words}", &result[..at])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::TAG;

    const WORK: WorkNumber = WorkNumber(1);

    #[test]
    fn a_result_longer_than_its_words_allow_is_given_cut_with_its_size_its_file_and_one_within_it_whole() {
        let words = ResultGiven {
            bound: 40,
            cut: "(Cut: {given} of {size} bytes; whole in {file}.)".to_string(),
        };
        let long = format!("{}é{}", "a".repeat(39), "b".repeat(60));
        // The 40th byte is inside a character, so the cut is before it.
        let size = long.len();
        assert_eq!(
            cut(&long, Some(&words), "/t/1.txt"),
            format!("{}\n\n(Cut: 39 of {size} bytes; whole in /t/1.txt.)", "a".repeat(39))
        );
        let short = "b".repeat(40);
        assert_eq!(cut(&short, Some(&words), "/t/1.txt"), short);
        // The size is the bytes the record holds, the whitespace at its end among them.
        let padded = format!("{}\n\n\n", "c".repeat(41));
        let said = format!("(Cut: 40 of {} bytes;", padded.len());
        assert!(cut(&padded, Some(&words), "/t/1.txt").contains(&said));
        // Words that give no bound give it whole, its end of whitespace left off.
        assert_eq!(cut(&format!("{long}\n"), None, "/t/1.txt"), long);
    }

    /// The flow of this repository, compiled against what its engine has.
    fn engineer() -> Compiled {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&[
                "answer",
                "asked",
                "dependencies",
                "head",
                "main",
                "session",
                "start",
                "tree",
                "words",
            ]),
            acts: names(&["start", "check", "merge", "deliver", "run_tests"]),
            tools: names(&["query"]),
            ..Default::default()
        };
        flow::compile(&flow::read(&dir).unwrap(), &has).unwrap()
    }

    /// A work that asked `query` once, with this result, and was given this result of the tests I1 asked for.
    fn work_with(tests: &str, query: &str) -> work::Work {
        let mut work = work::Work::default();
        let asked = work::Call::Asked(work::Ask::Tool {
            name: QUERY.to_string(),
            input: "{\"sql\": \"SELECT 1\"}".to_string(),
        });
        work.calls.insert(CallId(2), asked);
        work.results.insert(CallId(2), query.to_string());
        work.contents
            .insert((BlockId::from("i1_tested"), Seq(5)), tests.to_string());
        work
    }

    /// The words of `query` as the flow has them, given cut at 20 bytes, and of a run of tests, at 30.
    fn bounded() -> (BTreeMap<String, Tool>, ResultGiven) {
        let query = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../flow/agents/tools/query.toml"
        ));
        let mut query: Tool = toml::from_str(query).expect("the file of query reads");
        query.given_bytes = Some(20);
        let mut tests = crate::acts::words().run_tests.given;
        tests.bound = 30;
        (BTreeMap::from([(QUERY.to_string(), query)]), tests)
    }

    #[tokio::test]
    async fn a_cut_result_is_kept_whole_in_a_file_the_message_names_outside_the_worktree() {
        let temp = tempfile::tempdir().unwrap();
        let dir = files_dir(temp.path(), TAG, WORK);
        let whole = format!("head\n{}\nthe last line\n", "a line of the result\n".repeat(10));
        let query = format!("{{\"rows\": [{}{{\"n\": 1}}]}}", "{\"n\": 1},".repeat(8));
        let work = work_with(&whole, &query);
        let (tools, tests) = bounded();
        let flow = engineer();
        keep_wholes(&work, (&tools, &flow, &tests), &dir).await.unwrap();
        let file = dir.join("i1_tested-5.txt");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            whole,
            "the file holds the whole result"
        );
        assert!(dir.join("2.txt").is_file(), "a cut query result has its file too");
        let given = tested(&work, &flow, &tests, &dir);
        let of = &given[&(BlockId::from("i1_tested"), Seq(5))];
        assert!(of.contains(&file.display().to_string()), "{of}");
        assert!(!of.contains("the last line"), "the message is cut");
        let cut: serde_json::Value =
            serde_json::from_str(&super::given(&work, (&tools, &tests), &dir)[&CallId(2)]).unwrap();
        assert_eq!(cut["cut"]["file"], dir.join("2.txt").display().to_string());
        // Nothing of it is in the data directory the works' worktrees are in.
        assert!(file.starts_with(temp.path()));
    }

    // A result of `run_tests` asked as a tool, kept from before the cells asked for tests in a record, whose tool no
    // file defines now, is given cut as the tests' results are, its whole in a file, never whole however long. Well
    // under a second alone; no other test holds such a result.
    #[tokio::test]
    async fn a_result_of_a_tool_no_file_defines_now_is_given_cut_as_the_tests_results_are() {
        let temp = tempfile::tempdir().unwrap();
        let dir = files_dir(temp.path(), TAG, WORK);
        let mut work = work_with("short", "{\"rows\": []}");
        let asked = work::Call::Asked(work::Ask::Tool {
            name: "run_tests".to_string(),
            input: "{}".to_string(),
        });
        let whole = format!("{}the last line\n", "a line of the tests\n".repeat(10));
        work.calls.insert(CallId(3), asked);
        work.results.insert(CallId(3), whole.clone());
        let (tools, tests) = bounded();
        let flow = engineer();
        keep_wholes(&work, (&tools, &flow, &tests), &dir).await.unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("3.txt")).unwrap(), whole);
        let given = &super::given(&work, (&tools, &tests), &dir)[&CallId(3)];
        assert!(!given.contains("the last line"), "{given}");
        assert!(given.contains(&dir.join("3.txt").display().to_string()), "{given}");
    }

    #[tokio::test]
    async fn a_result_given_whole_has_no_file() {
        let temp = tempfile::tempdir().unwrap();
        let dir = files_dir(temp.path(), TAG, WORK);
        let work = work_with("short", "{\"rows\": []}");
        let (tools, tests) = bounded();
        let flow = engineer();
        keep_wholes(&work, (&tools, &flow, &tests), &dir).await.unwrap();
        assert!(!dir.exists());
        assert_eq!(
            tested(&work, &flow, &tests, &dir)[&(BlockId::from("i1_tested"), Seq(5))],
            "short"
        );
    }

    #[tokio::test]
    async fn the_sweep_removes_the_files_older_than_the_days_they_are_kept_and_the_directories_left_empty() {
        let temp = tempfile::tempdir().unwrap();
        let old = files_dir(temp.path(), TAG, WorkNumber(1));
        let young = files_dir(temp.path(), TAG, WorkNumber(2));
        for dir in [&old, &young] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("1.txt"), "x").unwrap();
        }
        let root = old.parent().unwrap();
        let now = std::time::SystemTime::now();
        sweep(root, now).await;
        assert!(
            old.join("1.txt").is_file() && young.join("1.txt").is_file(),
            "young files stay"
        );
        let later = now + std::time::Duration::from_secs((KEPT_DAYS + 1) * 24 * 3600);
        sweep(root, later).await;
        assert!(!old.exists() && !young.exists());
    }

    // A call of `run_tests` as a tool, in a work kept from before the cells asked for tests in a record, is answered,
    // where its run's cell asks for tests in the record, that it is asked for there now, in the words of the act's
    // file, so that the run goes on to write an entry; of a cell that does not, and a call named after another record
    // the cell writes, as any tool the engine does not have. Well under a second alone; no other test holds what such a
    // call is answered.
    #[tokio::test]
    async fn a_call_of_run_tests_as_a_tool_is_told_to_write_an_entry_where_its_cell_asks_for_tests_in_the_record() {
        let flow = Arc::new(engineer());
        let words = crate::acts::words().run_tests.as_a_tool;
        let tools = Tools::new(Querying::closed(), words.clone());
        let calls = [("I1", "run_tests"), ("T1", "run_tests"), ("I1", "q_and_a")];
        for (n, (cell, name)) in calls.into_iter().enumerate() {
            let (id, run) = (CallId(n as u64 + 1), decide::RunId(n as u64 + 1));
            let mut kept = work::Work::default();
            let at = kept.tick();
            kept.records.started.push(decide::Started {
                run,
                cell: cell.into(),
                at,
                because: Vec::new(),
                basis: BTreeMap::new(),
            });
            let first = store::First {
                title: "Greet the user".to_string(),
                level: None,
                project: None,
                depends_on: Vec::new(),
                tags: Vec::new(),
            };
            let call = ToolCall {
                work: WORK,
                flow: flow.clone(),
                run,
                call: id,
                name: name.to_string(),
                input: "{\"tests\": []}".to_string(),
                item: store::Item::new(store::ItemNumber(1), work::Time(0), first, Vec::new()),
                kept: Arc::new(kept),
            };
            let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
            tools.call(call, CallReply::new(WORK, id, to));
            let Some(service::Message::Happened {
                event: engine::Event::Done { result, .. },
                ..
            }) = said.recv().await
            else {
                panic!("the call is answered with its result");
            };
            match (cell, name) {
                ("I1", "run_tests") => assert_eq!(result, flow::agents::fill(&words, &[("name", "run_tests")])),
                _ => assert_eq!(result, format!("No such tool: {name}. The tools are: query.")),
            }
        }
    }

    #[tokio::test]
    async fn a_tool_the_engine_does_not_have_is_none_and_run_tests_is_no_tool() {
        let flow = Arc::new(engineer());
        let tools = Tools::new(Querying::closed(), String::new());
        for (n, name) in ["run_tests", "ci", "check_before"].into_iter().enumerate() {
            let id = CallId(n as u64 + 1);
            let first = store::First {
                title: "Greet the user".to_string(),
                level: None,
                project: None,
                depends_on: Vec::new(),
                tags: Vec::new(),
            };
            let call = ToolCall {
                work: WORK,
                flow: flow.clone(),
                run: decide::RunId(1),
                call: id,
                name: name.to_string(),
                input: String::new(),
                item: store::Item::new(store::ItemNumber(1), work::Time(0), first, Vec::new()),
                kept: Arc::new(work::Work::default()),
            };
            let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
            tools.call(call, CallReply::new(WORK, id, to));
            let Some(service::Message::Happened {
                event: engine::Event::Done { result, .. },
                ..
            }) = said.recv().await
            else {
                panic!("the call is answered with its result");
            };
            assert_eq!(result, format!("No such tool: {name}. The tools are: query."));
        }
    }
}
