// The items and the works as the pages read them, as `crates/web/src/works.rs` gives them: the types here follow
// those, field for field. Every time is in milliseconds since the start of 1970. A place (`at`) is the place of an
// entry in the order of what happened to a work; a version of a block is named by the place of the entry that wrote
// it.
//
// The routes:
// - `GET  /api/v1/backup`: how the copies of the database stand (`Copies`).
// - `GET  /api/v1/items`: every item with its work in brief (`Items`).
// - `POST /api/v1/items`: adds an item with its work (`NewItem`), and gives their numbers (`Added`).
// - `POST /api/v1/items/{item}/level`: puts an item at another level (`{ level }`), and says whether that changed it
//   (`ItemChanged`).
// - `POST /api/v1/items/{item}/title`: gives an item another title (`{ title }`), and says whether that changed it
//   (`ItemChanged`).
// - `POST /api/v1/items/{item}/dependencies`: makes an item depend on another or no longer (`DependencyChange`), while
//   its work has started no run, and says whether that changed it (`ItemChanged`).
// - `GET  /api/v1/levels`: the levels an item may be given, with what each is for (`Levels`).
// - `GET  /api/v1/projects`: the projects the installation names, in order of their names (`Projects`).
// - `GET  /api/v1/works/{work}`: one work, as it is now and step by step from its start (`WorkPage`).
// - `GET  /api/v1/works/{work}/versions/{block}/{at}`: the content of a version of a block (`VersionPage`).
// - `GET  /api/v1/works/{work}/runs/{run}/rounds/{number}`: one round in full (`RoundPage`).
// - `GET  /api/v1/works/{work}/runs/{run}/rounds/{number}/told`: what the round was told and what came back of it, as
//   its carrier kept them (`ToldPage`); no try where it kept nothing.
// - `GET  /api/v1/works/{work}/runs/{run}/rounds/{number}/doing`: what the round's agent was last seen doing, as its
//   carrier keeps it as it comes (`DoingPage`); read again every few seconds while the round is under way.
// - `GET  /api/v1/works/{work}/lines/{line}/sessions/{session}/system`: the system prompt the line's session of this
//   number, the line's first being none, began with (`SystemPage`).
// - `GET  /api/v1/works/{work}/calls/{call}`: a call with its result (`CallPage`).
// - `POST /api/v1/works/{work}/acts`: what the owner does to a work (`Act`); answers with no content, or with why it
//   was refused (`Refusal`) and a status of 4xx.
// - `GET  /api/v1/engine`: the engine as a whole, its slots and the pause of everything (`Engine`).
// - `POST /api/v1/engine/acts`: what the owner does to the engine as a whole (`EngineAct`); answers as an act on a work.
// - `GET  /api/v1/changes`: a stream of server-sent events, one each time something happened (`Changed`), on which a
//   page reads again what it shows.

import { read } from './api';
import type { Flow } from './api';

export type Person = 'owner' | 'architect';

/// Who paused a work: the owner, or the engine itself, as when a round failed or the work spent more than it may.
export type Pauser = 'owner' | 'engine';

/// The limit a run reached.
export type Limit = { of: 'returns'; sentBack: number } | { of: 'calls'; made: number };

/// What a call is: a tool the engine runs, a question for a person, a run at a limit that the owner lets go on, or
/// the engine's wait until a time before a round that failed in passing, the `failed`th in a row, is tried again.
export type Call =
  | { kind: 'tool'; name: string; input: string }
  | { kind: 'question'; who: Person; question: string }
  | { kind: 'atLimit'; limit: Limit }
  | { kind: 'wait'; until: number; failed: number };

/// The kind of cause of a round that failed, as what carried it out told it apart.
export type Failure =
  | 'passing'
  | 'unread'
  | 'flagged'
  | 'usageLimit'
  | 'cannotRun'
  | 'offline'
  | 'promptTooLong'
  | 'other';

/// When the rounds a wait holds may begin again: at a time, every work's; or one round tries at a time, where `look`
/// once a look at the network every half minute reaches it; or every round once a look at the host, every half minute
/// from a time, reaches it; or once the round trying now ends or is answered.
export type Until =
  | { kind: 'at'; at: number }
  | { kind: 'trial'; at: number; look: boolean }
  | { kind: 'looked'; at: number }
  | { kind: 'tried' };

/// A wait every work shares: a round of an agent failed for a cause no one work has, and no agent's round of any work
/// begins while it stands.
export interface Outage {
  /// The service whose rounds it holds: the agents', or the host's the engine's acts reach.
  service: 'agents' | 'host';
  failure: Failure;
  why: string;
  /// The round that failed so, the latest that did.
  work: number;
  run: number;
  round: number;
  /// When it ended.
  since: number;
  until: Until;
  /// Whether it holds rounds now: past its time it stands only until a round that began after it comes back, and
  /// holds nothing meanwhile.
  holds: boolean;
}

/// Who can make a precondition hold: the owner, a source outside the work (such as the items it depends on, as they
/// merge), or a cell, by what its next run writes.
export type Maker = { kind: 'owner' } | { kind: 'outside'; source: string } | { kind: 'cell'; cell: string };

/// What a work waits for.
export type Waited =
  /// The result of a call a run rests at: from this person, or from the engine where there is none.
  | { kind: 'call'; call: number; who: Person | null; what: Call }
  /// A precondition of the cell does not hold: the named test, the words shown for it, and each who can make it hold,
  /// by writing a block that keeps it from holding.
  | { kind: 'precondition'; test: string; words: string; by: Maker[] }
  /// The cell's last runs, this many in a row, were each for what a cell after it wrote.
  | { kind: 'limit'; runs: number }
  /// The cell's last runs, this many in a row, each asked for calls and dealt with nothing.
  | { kind: 'calls'; runs: number }
  /// A free slot: as many rounds of agents are under way as the engine has slots, and it gives the next that comes free
  /// in its order.
  | { kind: 'slot' }
  /// The wait every work shares: the cell's agent's round does not begin until it is over.
  | { kind: 'agents'; outage: Outage }
  /// The owner's answer to the latest version of this question, which the cell asked and waits for.
  | { kind: 'answer'; question: string };

/// What a work does now.
export type Now =
  | { kind: 'fault'; words: string }
  | { kind: 'merged' }
  | { kind: 'cancelled' }
  /// A round of this run of this cell is under way, since this time.
  | { kind: 'round'; cell: string; run: number; round: number; since: number }
  /// It is paused, and what it would do next is begin a round: by the owner, who gives no reason, or by the engine
  /// for this one.
  | { kind: 'paused'; by: Pauser; why: string | null }
  /// The owner's pause of everything holds it back, and what it would do next is begin a round; its own pause does not.
  | { kind: 'everything' }
  /// It waits at this cell, for each of these.
  | { kind: 'waits'; at: string; on: Waited[] }
  /// It is about to do something at this cell; a work is never left so between two things that happen to it.
  | { kind: 'next'; cell: string }
  /// Nothing new runs a cell, and nothing of it is under way or waited for: it goes on once someone writes what runs one.
  | { kind: 'idle' };

/// How a cell stands for a work: a round of its run is under way; its run is at rest; its run's last round failed,
/// and is given again once the work is let on; its condition is not met; it never finished a run; it has a record and
/// runs again when the walk reaches it; its record knows everything.
export type CellState = 'running' | 'resting' | 'failed' | 'skipped' | 'notYet' | 'again' | 'done';

export interface CellNow {
  cell: string;
  state: CellState;
  /// How many of its runs finished.
  finished: number;
  /// Its latest finished run.
  record: number | null;
  /// Its run that has neither finished nor been abandoned.
  open: number | null;
}

/// What rounds took.
export interface Spent {
  /// Millionths of a dollar.
  usdMicros: number;
  inputTokens: number;
  cachedTokens: number;
  outputTokens: number;
  /// How long the rounds were under way.
  workingMs: number;
  /// How many rounds ended.
  rounds: number;
}

/// A work in brief.
export interface Summary {
  number: number;
  item: number;
  flow: string;
  added: number;
  /// Only a note to a cell is answered; a round under way goes on.
  paused: boolean;
  now: Now;
  /// In the order of the walk.
  cells: CellNow[];
  spent: Spent;
  /// When something last happened to it.
  last: number | null;
  /// When its first round began; none before it has one. Absent from an engine that does not give it yet.
  from?: number | null;
  /// Where its time went, from its first round's begin to its last round's end, cut into equal cells, one character
  /// each: `m` an agent's rounds, `t` an engine cell's rounds or the tools' calls, `.` neither. Empty with no round;
  /// absent from an engine that does not give it yet.
  bar?: string;
  /// How many of its rounds were given back what the round before handed in; absent from an engine that does not give
  /// it yet.
  sentBack?: number;
}

export interface Item {
  number: number;
  title: string;
  /// The level it was given as it was added, which chooses what its agents' rounds run on; none for an item added
  /// before items had levels, whose rounds run at the default level.
  level: string | null;
  /// The project it belongs to, by the name the installation's settings give it; none for an item added before items
  /// had projects.
  project: string | null;
  added: number;
  dependsOn: number[];
  /// What it was as it was added, and each change made to it since: what it said before, and when it changed.
  history: ItemHistory;
  /// The one work done for it, which was added with it.
  work: Summary;
}

/// An item as it was added, and each change made to it since, in the order they were made.
export interface ItemHistory {
  first: { title: string; level: string | null; dependsOn: number[] };
  changes: ItemChange[];
}

/// A change made to an item, at this time.
export type ItemChange =
  | { kind: 'title'; time: number; title: string }
  | { kind: 'level'; time: number; level: string }
  | { kind: 'dependsOn'; time: number; item: number }
  | { kind: 'noLongerDependsOn'; time: number; item: number };

export interface Items {
  items: Item[];
  /// A wait every work shares, where one stands; the interface always says, as none where none stands, and a page's own
  /// made-up items may leave it out.
  outage?: Outage | null;
}

/// Why a cell runs: it has no record, or a block has another meaning than the one its record knows (`was`), which a
/// cell after it gave the block where `behind`.
export type Cause =
  | { kind: 'noRecord' }
  | { kind: 'changed'; block: string; was: number | null; behind: boolean }
  | { kind: 'results'; calls: number[] };

/// A block a cell reads, as the cell's record knows it and as it is now.
export interface Input {
  block: string;
  current: number | null;
  known: number | null;
  changed: boolean;
}

export interface CellDetail {
  cell: string;
  /// Why the cell would run were the walk to reach it; empty where it would not.
  needs: Cause[];
  inputs: Input[];
}

export type Writer = { kind: 'person' } | { kind: 'outside' } | { kind: 'run'; run: number; cell: string };

/// A version of a block: one write of it.
export interface Version {
  at: number;
  time: number;
  by: Writer;
  /// The version at which the meaning it has last changed: itself, or an earlier one.
  meaning: number;
  value: string | null;
  /// How long its content is, in characters.
  chars: number;
}

export interface Block {
  name: string;
  versions: Version[];
}

export type Unmet =
  | { kind: 'asksNothing' }
  | { kind: 'notAnOutput'; block: string }
  | { kind: 'writtenTwice'; block: string }
  | { kind: 'noValue'; block: string }
  | { kind: 'notAValue'; block: string; value: string }
  | { kind: 'notAChoice'; block: string }
  | { kind: 'notHandedIn'; block: string }
  | { kind: 'unreadable'; why: string }
  | { kind: 'notRunBy'; block: string }
  | { kind: 'dealtWithTwice'; block: string }
  | { kind: 'dealtWithNothing' }
  | { kind: 'writtenOver'; block: string; version: number }
  | { kind: 'choiceChanged'; block: string }
  | { kind: 'asksAndWorks' }
  | { kind: 'notATrigger'; name: string }
  | { kind: 'notATool'; name: string }
  | { kind: 'unchangedNotWritten'; block: string }
  | { kind: 'questionTaken'; name: string }
  | { kind: 'noSuchQuestion'; name: string }
  | { kind: 'questionTwice'; name: string }
  | { kind: 'notAnOption'; name: string }
  | { kind: 'notAnswerable'; name: string };

/// What a round is given: the cell's task, the results of the calls the round before asked for, or why what the
/// round before handed in was sent back.
export type Given = { kind: 'task' } | { kind: 'results'; calls: number[] } | { kind: 'sentBack'; unmet: Unmet[] };

export type Ask = { kind: 'tool'; name: string; input: string } | { kind: 'question'; who: Person; question: string };

/// A block as a round hands it in, without its content.
export interface Handed {
  block: string;
  /// Whether the run says the change matters to the cells that read the block.
  matters: boolean;
  value: string | null;
  chars: number;
}

/// What came back at the end of a round: calls asked for, blocks handed in, or nothing, the round having failed for
/// this reason, of this kind of cause; `resets` says when the usage limit it reached resets, where its report said.
export type Back =
  | { kind: 'asked'; asks: Ask[] }
  | { kind: 'handedIn'; blocks: Handed[] }
  | { kind: 'failed'; why: string; failure: Failure; resets: number | null };

export interface Round {
  /// The first being one.
  number: number;
  began: number;
  /// None while it is under way.
  ended: number | null;
  given: Given;
  /// None while it is under way, and for a round that ended after its run was abandoned.
  back: Back | null;
  spent: Spent | null;
  /// The context it ended with, in tokens; none while it is under way and where it is not known.
  context: number | null;
  /// How often its session was compacted during it.
  compactions: number;
  /// The session of its line it runs in; none for a round of a cell on no line, as an engine's act is.
  session: RoundSession | null;
  /// Each try of it the records hold, in order; none for a try that began before tries were recorded.
  tries: RoundTry[];
}

/// The session of its line a round runs in: which of the line's sessions, as the records count them, the line's first
/// being none.
export interface RoundSession {
  line: string;
  number: number;
}

/// A try of a round, as the records hold it: what it ran on, and each time something was filled into its session,
/// what was.
export interface RoundTry {
  attempt: number;
  ran: RanOn;
  filled: Fill[][];
}

/// One thing filled into a session, as the flow's files name it: a part of the message the session keeps, by its
/// `part`; a version of a datum of the cell's context, whole, or as what changed since an earlier one; the results of
/// calls; why what a round of the run returned was sent back.
export type Fill =
  | { kind: 'part'; part: string }
  | { kind: 'whole'; block: string; version: number }
  | { kind: 'change'; block: string; version: number; since: number }
  | { kind: 'results'; calls: number[] }
  | { kind: 'sentBack'; round: number };

export interface CallRow {
  call: number;
  what: Call;
  made: number;
  done: number | null;
  /// How long its result is, in characters.
  chars: number | null;
}

export type RunEnd =
  /// It finished and wrote these blocks: each is the version of its block named by `at`.
  | { kind: 'finished'; at: number; time: number; writes: Handed[] }
  | { kind: 'abandoned'; at: number; time: number };

/// One run of a cell.
export interface Run {
  run: number;
  cell: string;
  at: number;
  time: number;
  because: Cause[];
  /// The version of each block it reads or may write that it started from.
  basis: { block: string; version: number | null }[];
  rounds: Round[];
  calls: CallRow[];
  /// None while it is open.
  end: RunEnd | null;
  spent: Spent;
}

/// One thing that happened to a work. A run stands where it started.
export type Step =
  | ({ kind: 'run' } & Run)
  | {
      kind: 'wrote';
      at: number;
      time: number;
      block: string;
      by: Writer;
      /// Whether the write changed the block's meaning.
      changed: boolean;
      value: string | null;
      chars: number;
    }
  | { kind: 'letGo'; at: number; time: number; cell: string }
  /// The work was paused: by the owner, or by the engine for this reason.
  | { kind: 'paused'; at: number; time: number; by: Pauser; why: string | null }
  | { kind: 'resumed'; at: number; time: number }
  /// The work's item was cancelled, for this reason where whoever cancelled it gave one.
  | { kind: 'cancelled'; at: number; time: number; why: string | null }
  /// A new session of the line began, or begins with the line's next round.
  | { kind: 'session'; at: number; time: number; line: string; why: SessionWhy };

/// Why a line began a new session: its context was past the limit; its rounds run on another model or under another
/// account than its session's ran on; the owner asked; the service's safety check flagged the line's last round; the
/// agents' program had lost the one before; the agents' program compacted it during a round.
export type SessionWhy =
  | { kind: 'limit'; context: number; limit: number }
  | { kind: 'model'; was: string }
  | { kind: 'account' }
  | { kind: 'owner' }
  | { kind: 'flagged' }
  | { kind: 'lost' }
  | { kind: 'compacted' };

/// One work: as it is now, and step by step from its start.
export interface WorkPage {
  summary: Summary;
  /// The title of its item.
  title: string;
  /// The level of its item, as `Item` has it.
  level: string | null;
  /// The project of its item, as `Item` has it.
  project: string | null;
  /// What its item was as it was added, and each change made to it since, as `Item` has them.
  history: ItemHistory;
  /// The flow it runs on.
  flow: Flow;
  /// In the order of the walk.
  cells: CellDetail[];
  /// Every block that has a version, by name.
  blocks: Block[];
  /// In the order it happened.
  steps: Step[];
}

export interface VersionPage {
  block: string;
  at: number;
  content: string;
}

export interface CallPage {
  call: number;
  what: Call;
  result: string | null;
}

/// One round in full: the results it was given, and the content of each block it handed in.
export interface RoundPage {
  run: number;
  number: number;
  results: CallPage[];
  handed: { block: string; content: string }[];
}

/// What came back of a try of a round: nothing that was kept (it is under way, or was stopped, never got as far as its
/// program, or its program printed no result); what it returned, only as much as the page shows of what the agent's
/// program printed; that the program said it failed, and why; or why what was kept of it could not be read.
export type TryBack =
  | { kind: 'nothing' }
  | {
      kind: 'returned';
      /// The agent's closing words; none where they are the structured return itself.
      text: string | null;
      /// The structured return, as pretty-printed JSON.
      structured: string | null;
      turns: number | null;
      durationMs: number | null;
      /// The models it was answered by, by name.
      models: string[];
      /// The tokens it wrote to the prompt cache, as the program printed them; the records count them among those
      /// not read from the cache. Absent from an engine that does not give it yet.
      cacheCreationInputTokens?: number | null;
    }
  | { kind: 'failed'; why: string }
  | { kind: 'unreadable'; why: string };

/// One try of a round: the message it was told, as it was written up to where it was cut, and what came back.
export interface Try {
  message: string;
  /// The message's whole length in bytes, where it was cut before its end.
  cut: number | null;
  back: TryBack;
  /// What it ran on as it began; none where its carrier kept nothing of it, as of an act of the engine's own.
  ran: RanOn | null;
}

/// What a try of an agent's round ran on, as the work's records say it was when the try began: its level, that level's
/// model, and the reasoning effort its cell sets.
export interface RanOn {
  level: string;
  model: string;
  effort: string | null;
}

/// The levels an item may be given, lowest first, with what work each is for, the one an item is given where it is
/// given none, and the highest an item may be at.
export interface Levels {
  levels: { name: string; desc: string }[];
  default: string;
  highest: string;
}

/// The projects the installation names, in order of their names.
export interface Projects {
  names: string[];
}

/// Whether a change of an item changed it: it was so already where not, as when it was at that level already.
export interface ItemChanged {
  changed: boolean;
}

/// A change of what an item depends on.
export type DependencyChange = { kind: 'add'; item: number } | { kind: 'remove'; item: number };

/// What a round was told and what came back of it, each try in order, as its carrier kept them; no try where it kept
/// nothing, as of a round of the engine's own acts or of a script. Tries past what is read of a round are left out,
/// and counted.
export interface ToldPage {
  tries: Try[];
  leftOut: number;
}

/// The system prompt a line's session began with, as it was kept with the session.
export interface SystemPage {
  line: string;
  session: number;
  text: string;
}

/// A thing an agent was seen doing: a tool it used, on what its input names; what it said; a compaction of its session.
export type Doing =
  | { kind: 'tool'; name: string; on: string | null }
  | { kind: 'said'; text: string }
  | { kind: 'compacting' };

/// What the agent of a round was last seen doing: the last few things its latest try did, the latest last.
export interface DoingPage {
  doing: Doing[];
}

/// What the owner does to a work.
export type Act =
  /// Writes a block people write: what the item is about, an opinion for a cell, a choice (with its value).
  | { kind: 'write'; block: string; content: string; value: string | null }
  | { kind: 'letGo'; cell: string }
  | { kind: 'pause' }
  | { kind: 'resume' }
  /// Cancels the work's item, saying why where a reason is given, such as the item it was joined into.
  | { kind: 'cancel'; why?: string | null }
  /// Gives a call that waits for a person its result.
  | { kind: 'result'; call: number; result: string }
  /// Answers a version of a question, taking an option counted from one, saying why where the owner does.
  | { kind: 'answer'; question: string; version: number; option: number; why?: string | null }
  /// Has the line begin a new session at its next round.
  | { kind: 'newSession'; line: string };

export interface NewItem {
  title: string;
  /// The level it is given; with none, the level the installation adds an item at where it is given none.
  level?: string | null;
  /// The project it belongs to, one the installation names.
  project: string;
  dependsOn: number[];
  /// The flow its work runs on.
  flow: string;
  /// What is written for it as it is added, each a block people write.
  writes: { block: string; content: string; value: string | null }[];
}

/// The engine as a whole: its slots, and the pause of everything, while it holds.
export interface Engine {
  slots: number;
  taken: number;
  paused: EverythingPaused | null;
}

/// The pause of everything: when it began, and who set it, the owner, or the engine as it started with works going on,
/// which holds everything until the owner has seen how things stand and lifts it as they lift their own.
export interface EverythingPaused {
  since: number;
  by: Pauser;
}

/// What the owner does to the engine as a whole: pauses everything, or lifts the pause of everything, theirs or the
/// engine's.
export type EngineAct = { kind: 'pause' } | { kind: 'resume' };

export interface Added {
  item: number;
  work: number;
}

/// Why something asked of the interface was refused, in words. `kept` is there where the engine stopped as it was
/// asked: whether what was asked was kept before it stopped, and so is not to be asked again.
export interface Refusal {
  error: string;
  kept?: boolean;
}

/// Something happened: these works grew, the items changed where `items`, and the copies of the database may have
/// changed where `copies`.
export interface Changed {
  works: number[];
  items: boolean;
  copies: boolean;
}

/// Sends something to the interface; gives why where it is refused.
export async function send<T>(path: string, body: unknown): Promise<T | null> {
  const response = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const refusal = (await response.json().catch(() => null)) as Refusal | null;
    throw new Error(refusal?.error ?? `${path} answered ${response.status}`);
  }
  return response.status === 204 ? null : ((await response.json()) as T);
}

/// How the copies of the database stand: the engine works it out from its records of them and the time on its clock.
export type CopiesState =
  | { kind: 'ok' }
  /// Kept on the machine alone, which shares the disk they are kept against losing.
  | { kind: 'localOnly' }
  /// This many copies kept were not yet uploaded off the machine.
  | { kind: 'pending'; n: number }
  /// The newest copy is older than a day and an hour; `since` is when it was taken, none where there is none.
  | { kind: 'stale'; since: number | null }
  /// A copy is dated more than a day ahead of now: the clock is wrong.
  | { kind: 'clock' }
  /// The last step that failed, and nothing done since mends it.
  | { kind: 'failed'; at: number; step: string; why: string };

export interface Copies {
  state: CopiesState;
  /// The time now by the engine's clock.
  now: number;
  lastTaken: number | null;
  lastUploaded: number | null;
  pending: number;
  lastFailure: { at: number; step: string; why: string } | null;
}

export const loadCopies = () => read<Copies>('/api/v1/backup');
export const loadItems = () => read<Items>('/api/v1/items');
export const loadWork = (work: number) => read<WorkPage>(`/api/v1/works/${work}`);
export const loadVersion = (work: number, block: string, at: number) =>
  read<VersionPage>(`/api/v1/works/${work}/versions/${encodeURIComponent(block)}/${at}`);
export const loadRound = (work: number, run: number, round: number) =>
  read<RoundPage>(`/api/v1/works/${work}/runs/${run}/rounds/${round}`);
export const loadTold = (work: number, run: number, round: number) =>
  read<ToldPage>(`/api/v1/works/${work}/runs/${run}/rounds/${round}/told`);
export const loadDoing = (work: number, run: number, round: number) =>
  read<DoingPage>(`/api/v1/works/${work}/runs/${run}/rounds/${round}/doing`);
export const loadSystem = (work: number, line: string, session: number) =>
  read<SystemPage>(`/api/v1/works/${work}/lines/${encodeURIComponent(line)}/sessions/${session}/system`);
export const loadCall = (work: number, call: number) => read<CallPage>(`/api/v1/works/${work}/calls/${call}`);
export const loadLevels = () => read<Levels>('/api/v1/levels');
export const loadProjects = () => read<Projects>('/api/v1/projects');
export const act = (work: number, what: Act) => send<null>(`/api/v1/works/${work}/acts`, what);
export const addItem = (item: NewItem) => send<Added>('/api/v1/items', item);
export const setLevel = (item: number, level: string) => send<ItemChanged>(`/api/v1/items/${item}/level`, { level });
export const setTitle = (item: number, title: string) => send<ItemChanged>(`/api/v1/items/${item}/title`, { title });
export const changeDependencies = (item: number, change: DependencyChange) =>
  send<ItemChanged>(`/api/v1/items/${item}/dependencies`, change);
export const loadEngine = () => read<Engine>('/api/v1/engine');
export const actOnEngine = (what: EngineAct) => send<null>('/api/v1/engine/acts', what);
