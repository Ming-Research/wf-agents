// A round, as the owner reads it, in the order the owner reads it. First where the work came from, who ran the round
// (the cell, and for an agent its line, level, model, effort, whether it started or continued its line's session,
// its time and cost) and where the work went, one under another, the cells and the rounds before and after it links
// that open them. For an agent's round, then: its task, folded; its inputs, one row for each input of its cell by the
// cell's own numbering, each marked sent this round or already in the session, what it holds under it drawn by its
// shape (open when sent this round, folded and read on opening when the session held it), those with nothing this
// round kept in their place and struck through; and what it handed back, drawn by its shape too, with the row it
// took. For a step that is not an agent's, what it came to and what it read; for tools, what each was asked and gave.
// It ends with the message as sent and the return as handed back, raw and folded, for the final check. Nothing else is
// shown as a tree of JSON or as escaped JSON text.

import { type ReactNode, useContext, useState } from 'react';

import type { CompiledCell, CompiledInput, JsonValue, RoundDetail, RoundSummary, ToolRun } from '../api';
import { Loaded, OutcomeBadge } from '../components';
import { numbered } from '../flow/CellPanel';
import { type Flow, FlowContext } from '../flow/context';
import { targetWords } from '../flow/words';
import { cost, duration, roundName, time } from '../format';
import { getJson, getText } from '../http';
import { Acceptance, Approaches, Deviation, Design, Field, Fold, Grounds, Measure, Understanding } from '../item/Shapes';
import { empty, isObj, label, Labelled, type Obj, str, words } from '../labelled';
import { useResource } from '../live';
import { Markdown } from '../markdown';
import { srcText, t } from '../text';
import { AskView, DesignCardView } from './Decide';

/// A message, as the engine writes one: its given inputs by address, its task and its list of inputs to work from.
type Parsed = { given: Map<string, string>; task: string | null; workFrom: string[] };

export function parseMessage(message: string): Parsed {
  const given = new Map<string, string>();
  const task: string[] = [];
  const workFrom: string[] = [];
  let part: 'head' | 'given' | 'task' | 'from' | 'other' = 'head';
  let address: string | null = null;
  let content: string[] = [];
  const close = () => {
    if (address !== null) given.set(address, content.join('\n').trim());
    address = null;
    content = [];
  };
  for (const line of message.split('\n')) {
    if (line.startsWith('## ')) {
      close();
      const h = line.slice(3);
      part = h.startsWith('Given') ? 'given' : h === 'Task' || h === 'What to do' ? 'task' : h === 'Work from' ? 'from' : 'other';
      continue;
    }
    if (part === 'given' && line.startsWith('### ')) {
      close();
      address = line.slice(4).trim();
      continue;
    }
    if (part === 'given' && address !== null) content.push(line);
    else if (part === 'task') task.push(line);
    else if (part === 'from' && line.trim() !== '') workFrom.push(line);
  }
  close();
  return { given, task: task.join('\n').trim() || null, workFrom };
}

/// The addresses the list to work from gives each numbered input this round, by the input's number.
export function addressesByNumber(workFrom: string[]): Map<number, string[]> {
  const out = new Map<number, string[]>();
  for (const line of workFrom) {
    const hit = /^(\d+)\. (.*)$/.exec(line);
    if (hit) out.set(Number(hit[1]), hit[2].split(', '));
  }
  return out;
}

/// What an address names, in words, for an address no input of the cell gives: a return format, a rule, the worktree.
function addressWords(a: string): string {
  const colon = a.indexOf(':');
  const [kind, rest] = colon > 0 && !a.startsWith('R') ? [a.slice(0, colon), a.slice(colon + 1)] : ['', a];
  switch (kind) {
    case 'format':
      return rest === 'common' ? t('addr_common_format') : t('addr_cell_format', { cell: rest });
    case 'inputs':
      return t('addr_cell_inputs', { cell: rest });
    case 'rule':
      return t('addr_rule', { name: rest });
    case 'work':
      return t('addr_work');
    case 'item':
      return t('addr_item');
    case 'config':
      return t('addr_setting', { name: words(rest) });
    case 'ij':
      return t('addr_interjection');
    case 'design_card':
      return t('addr_design_card', { id: rest });
    case 'ask':
      return t('addr_ask', { id: rest });
  }
  return a;
}

/// Where an input's address says its content comes from, in words: a round and its cell, the engine, the item, a
/// setting.
export function sourceWords(a: string, title: (cell: string) => string): string {
  const colon = a.indexOf(':');
  if (colon > 0 && !a.startsWith('R')) {
    const kind = a.slice(0, colon);
    const rest = a.slice(colon + 1);
    switch (kind) {
      case 'item':
        return t('src_item_n', { id: rest });
      case 'design_card':
        return t('addr_design_card', { id: rest });
      case 'ask':
        return t('addr_ask', { id: rest });
      case 'ij':
        return t('src_ij_n', { id: rest });
      case 'config':
        return t('src_setting', { name: words(rest) });
    }
    return addressWords(a);
  }
  const [round, cell] = a.split('·');
  if (cell === 'engine') return t('src_engine_for', { round });
  return t('src_from_round', { round, cell: title(cell) });
}

/// An input by a short name: its own, or that of where it comes from.
export function shortName(x: CompiledInput): string {
  // An input written without an id has none in the compiled flow either; one every agent's cell has goes by its name.
  if (x.name) return x.name;
  if (x.id) return words(x.id);
  const s = x.from[0];
  if (s === undefined) return '';
  if (Array.isArray(s)) return words(s[1].split('.').pop()?.replace(/\[.*\]$/, '') ?? s[1]);
  if (s === 'self') return 'self';
  if ('config' in s) return words(s.config);
  if ('db' in s) return words(s.db);
  if ('engine' in s) return words(s.engine);
  return words(s.recorded);
}

/// A model's name as people say it: `claude-opus-5-5` is Opus 5.5; any other name as it is.
export function modelName(model: string): string {
  const m = /^claude-([a-z]+)-(\d+)-(\d+)$/.exec(model);
  return m ? `${m[1].charAt(0).toUpperCase()}${m[1].slice(1)} ${m[2]}.${m[3]}` : model;
}

/// Rows written one JSON object to a line, as a query gives them: the objects; none for a text that is not so.
export function jsonLines(text: string): Obj[] | null {
  const lines = text.split('\n').filter((l) => l.trim() !== '');
  if (lines.length === 0 || !lines.every((l) => l.trimStart().startsWith('{'))) return null;
  const rows: Obj[] = [];
  for (const l of lines) {
    try {
      const v = JSON.parse(l) as JsonValue;
      if (!isObj(v)) return null;
      rows.push(v);
    } catch {
      return null;
    }
  }
  return rows;
}

/// A text that holds JSON, as the value it holds; none for a text that does not.
function parsed(text: string): JsonValue | undefined {
  const s = text.trim();
  if (!/^[[{"]/.test(s)) return undefined;
  try {
    return JSON.parse(s) as JsonValue;
  } catch {
    return undefined;
  }
}

/// Objects of plain values, as a table with a column for each field any of them has.
function Rows({ rows }: { rows: Obj[] }) {
  const cols = [...new Set(rows.flatMap((r) => Object.keys(r)))];
  return (
    <div className="rows-scroll">
      <table className="list rows">
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c}>{words(c)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              {cols.map((c) => (
                <td key={c}>
                  <Content value={r[c]} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/// A value as it reads, whatever its shape: rows as a table (a query's rows, one object to a line, too), an object as
/// its fields, a list as a list, a text as text with its markdown drawn; a text that holds JSON as the value it holds.
export function Content({ value }: { value: JsonValue | undefined }) {
  if (value === undefined || value === null) return null;
  if (typeof value === 'string') {
    const rows = jsonLines(value);
    if (rows !== null) return <Rows rows={rows} />;
    const inner = parsed(value);
    if (inner !== undefined && typeof inner !== 'string') return <Content value={inner} />;
    return <Markdown text={value} />;
  }
  if (typeof value === 'number' || typeof value === 'boolean') return <span>{String(value)}</span>;
  if (Array.isArray(value)) {
    if (value.length > 0 && value.every(isObj)) return <Rows rows={value.filter(isObj)} />;
    return (
      <ul className="labelled-list">
        {value.map((v, i) => (
          <li key={i}>
            <Content value={v} />
          </li>
        ))}
      </ul>
    );
  }
  const fields = Object.entries(value).filter(([, v]) => !empty(v));
  return (
    <div className="labelled">
      {fields.map(([k, v]) => (
        <div key={k} className="labelled-field">
          <span className="decision-label">{label(k)}</span>
          <Content value={v} />
        </div>
      ))}
    </div>
  );
}

/// A cell, as a link that brings it into view on the chart and opens it.
function CellLink({ f, id }: { f: Flow | null; id: string }) {
  const c = f?.cell(id);
  const name = c ? `${id} ${c.title}` : id;
  if (f === null) return <span className="path-cell">{name}</span>;
  return (
    <button type="button" className="link path-cell" onClick={() => f.open({ kind: 'cell', cell: id })}>
      <strong>{id}</strong> {c?.title ?? ''}
    </button>
  );
}

/// A round, as a link that opens it.
function RoundLink({ f, id, children }: { f: Flow | null; id: number; children: ReactNode }) {
  if (f === null) return <span className="muted">{children}</span>;
  return (
    <button type="button" className="link muted" onClick={() => f.open({ kind: 'round', round: id })}>
      {children}
    </button>
  );
}

/// The row a round took, as a chip.
function RowChip({ row }: { row: string }) {
  return <span className="row-chip">{words(row)}</span>;
}

/// Who did the round's work, in a few words: for an agent its line, level, model, effort, whether it started or
/// continued its line's session, its time and cost; else the engine, the tools or the owner.
function runWords(d: RoundDetail, cell: CompiledCell | undefined): string {
  const r = d.round;
  if (d.message !== null) {
    const call = isObj(d.call) ? d.call : {};
    const session = isObj(call.session) ? call.session : {};
    return [
      r.line,
      d.level !== null ? t('run_level', { level: d.level }) : null,
      r.model !== null ? modelName(r.model) : null,
      d.effort !== null ? t('run_effort', { effort: d.effort }) : null,
      'started' in session ? t('run_started') : 'resumed' in session ? t('run_continued') : null,
      r.ended_at !== null ? duration(r.ended_at - r.started_at) : null,
      r.cost_usd !== null ? cost(r.cost_usd) : null,
    ]
      .filter((x) => x !== null && x !== '')
      .join(' · ');
  }
  if (r.not_asked !== null) return t('run_not_asked', { need: words(r.not_asked) });
  if (cell?.kind === 'tool') return t('happened_tools');
  if (cell?.kind === 'owner') return t('happened_owner');
  return t('happened_engine', { step: cell?.step ?? '' });
}

/// Where the work came from, who ran the round and where the work went, one under another so that it never wraps.
function Path({ d, rounds, f }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null }) {
  const r = d.round;
  const cell = r.cell !== null ? f?.cell(r.cell) : undefined;
  const at = rounds.findIndex((x) => x.n === r.n);
  const before = at > 0 ? rounds[at - 1] : undefined;
  const after = at >= 0 && at + 1 < rounds.length ? rounds[at + 1] : undefined;
  const target = r.transition !== null && cell !== undefined ? cell.rows.find((x) => x.name === r.transition)?.to : undefined;
  const progress = d.message === null ? r.progress : null;
  return (
    <div className="path">
      {before !== undefined && before.cell !== null ? (
        <div className="path-end">
          <span className="path-label">{t('path_from')}</span>
          <CellLink f={f} id={before.cell} />
          {before.transition !== null ? <RowChip row={before.transition} /> : null}
          <RoundLink f={f} id={before.n}>
            ← {roundName(d.work, before.n)}
          </RoundLink>
        </div>
      ) : null}
      <div className="path-run">
        <div>{r.cell !== null ? <CellLink f={f} id={r.cell} /> : null}</div>
        <div>{runWords(d, cell)}</div>
        {progress ? <div className="muted">{progress}</div> : null}
        {r.outcome !== 'took' ? (
          <div>
            <OutcomeBadge outcome={r.outcome} />{' '}
            {r.outcome === 'running' ? duration(Date.now() - r.started_at) : (r.why ?? '')}
          </div>
        ) : null}
      </div>
      {r.outcome === 'took' && (after?.cell != null || target !== undefined) ? (
        <div className="path-end">
          <span className="path-label">{t('path_to')}</span>
          {r.transition !== null ? <RowChip row={r.transition} /> : null}
          {after?.cell != null ? (
            <CellLink f={f} id={after.cell} />
          ) : target !== undefined ? (
            (targetWords(target) ?? <CellLink f={f} id={target} />)
          ) : null}
          {after !== undefined ? (
            <RoundLink f={f} id={after.n}>
              {roundName(d.work, after.n)} →
            </RoundLink>
          ) : null}
        </div>
      ) : r.outcome !== 'took' && after !== undefined && after.cell !== null ? (
        <div className="path-end">
          <span className="path-label">{t('path_next')}</span>
          <CellLink f={f} id={after.cell} />
          <RoundLink f={f} id={after.n}>
            {roundName(d.work, after.n)} →
          </RoundLink>
        </div>
      ) : null}
      <div className="path-when muted">
        {r.ended_at !== null ? t('happened_at', { time: time(r.ended_at) }) : null}
        {r.compactions > 0 ? ` ${t('happened_compacted')}` : null}
      </div>
    </div>
  );
}

/// One input of the cell, by its number, as the round had it: its short name, and each of its addresses this round as
/// where it comes from, sent this round or already in the session; one with nothing this round in its place, struck
/// through.
type InputRow = { no: number; x: CompiledInput; addresses: Array<{ address: string; sent: boolean }> };

/// The cell's numbered inputs as the round had them, in the cell's order.
export function inputRows(c: CompiledCell | undefined, workFrom: string[], sent: Map<string, boolean>): InputRow[] {
  if (c === undefined) return [];
  const by = addressesByNumber(workFrom);
  return numbered(c).flatMap(({ x, no }) =>
    no === null ? [] : [{ no, x, addresses: (by.get(no) ?? []).map((address) => ({ address, sent: sent.get(address) ?? false })) }],
  );
}

function Chip({ sent }: { sent: boolean }) {
  return <span className={`chip ${sent ? 'sent-now' : 'in-session'}`}>{t(sent ? 'input_sent_now' : 'input_in_session')}</span>;
}

/// The counts of what a work did, as the cells' counters keep them, each in the flow's words with the project's limit
/// beside it.
function Counters({ rows, f, limits }: { rows: Obj[]; f: Flow | null; limits: { [key in string]: number } }) {
  return (
    <ul className="counters">
      {rows.map((r, i) => {
        const name = str(r.counter);
        const desc = f?.g.counters.find((k) => k.name === name)?.desc ?? words(name);
        const limit = limits[name];
        return (
          <li key={i}>
            <strong>{str(r.times) === '1' ? t('counter_once') : t('counter_times', { n: str(r.times) })}</strong>
            {limit !== undefined ? <span className="muted"> {t('counter_limit', { n: limit })}</span> : null} {desc}
          </li>
        );
      })}
    </ul>
  );
}

/// What an input held under one address, drawn by its shape: the work's counters in words beside the project's
/// limits, rows as a table, text as text.
function Held({ address, value, f, limits }: {
  address: string;
  value: JsonValue;
  f: Flow | null;
  limits: RoundDetail['limits'];
}) {
  const v = typeof value === 'string' ? (parsed(value) ?? value) : value;
  if (address.endsWith('·engine·work_counters') && Array.isArray(v) && v.every(isObj)) {
    return <Counters rows={v.filter(isObj)} f={f} limits={limits} />;
  }
  return <Content value={v} />;
}

/// A return as the addresses of its values read it: the one form it holds beside the rest.
function unwrapped(output: JsonValue | null, within: string): JsonValue | null {
  if (!isObj(output)) return output;
  const form = output[within];
  return isObj(form) ? { ...output, ...form } : output;
}

/// The value at a path of a return, as an address names it (`end.test_changes[added]`): a branch keeps only what is of
/// that branch, and the values at a path inside a list are a list.
export function atPath(value: JsonValue | null | undefined, path: string): JsonValue | undefined {
  let v: JsonValue | undefined = value ?? undefined;
  for (const seg of path.split('.')) {
    const hit = /^([^[]+)(?:\[(.+)\])?$/.exec(seg);
    if (hit === null || v === undefined) return undefined;
    const [, name, branch] = hit;
    const of = (x: JsonValue): JsonValue | undefined => (isObj(x) ? x[name] : undefined);
    v = Array.isArray(v) ? v.map(of).filter((x): x is JsonValue => x !== undefined) : of(v);
    if (branch !== undefined && v !== undefined) {
      const mine = (x: JsonValue) => isObj(x) && Object.values(x).some((y) => y === branch);
      v = Array.isArray(v) ? v.filter(mine) : mine(v) ? v : undefined;
    }
  }
  return v;
}

/// What an input already in the session held: found by the hash the round's record names it with, as the round of the
/// work that gave it gave it, or as the session's own return produced it; for a round recorded before its inputs had
/// hashes, given in an earlier round of the same session under its address, or read from the return the address
/// names. None when it is not found.
async function findHeld(
  address: string,
  hash: string,
  d: RoundDetail,
  rounds: RoundSummary[],
  within: string,
): Promise<JsonValue | null> {
  if (hash !== '') {
    const url = `/api/v1/works/${d.work}/contents/${hash}?address=${encodeURIComponent(address)}`;
    return getJson<JsonValue>(url).catch(() => null);
  }
  const earlier = rounds
    .filter((r) => r.n < d.round.n && r.line !== null && r.line === d.round.line)
    .reverse()
    .slice(0, 16);
  for (const r of earlier) {
    const x = await getJson<RoundDetail>(roundUrl(d.work, r.n));
    const given = parseMessage(x.message ?? '').given.get(address);
    if (given !== undefined) return given;
  }
  const m = /^R(\d+)-(\d+)·([^·]+)·(.+)$/.exec(address);
  if (m !== null && m[3] !== 'engine') {
    const x = await getJson<RoundDetail>(roundUrl(Number(m[1]), Number(m[2])));
    return atPath(unwrapped(x.output, within), m[4]) ?? null;
  }
  return null;
}

/// An input the session already held, folded under `summary`: what it holds is read from where the session got it
/// when it is opened.
function InSession({ address, d, rounds, f, summary }: {
  address: string;
  d: RoundDetail;
  rounds: RoundSummary[];
  f: Flow | null;
  summary?: ReactNode;
}) {
  const [value, setValue] = useState<JsonValue | null | undefined>(undefined);
  const hash = d.conditional.find((c) => c.address === address)?.hash ?? '';
  const toggle = (open: boolean) => {
    if (!open || value !== undefined) return;
    findHeld(address, hash, d, rounds, f?.g.within ?? 'outcome').then(setValue, () => setValue(null));
  };
  return (
    <details className="input-held" onToggle={(e) => toggle(e.currentTarget.open)}>
      <summary className={summary === undefined ? 'muted' : undefined}>{summary ?? t('input_open')}</summary>
      {value === undefined ? (
        <p className="muted">{t('loading')}</p>
      ) : value === null ? (
        <p className="muted">{t('input_not_found')}</p>
      ) : (
        <Held address={address} value={value} f={f} limits={d.limits} />
      )}
    </details>
  );
}

/// An agent's round: its task, folded, and its inputs, one row for each input of its cell by the cell's numbering,
/// what was sent this round open under its row and what the session already held folded there, an input with
/// nothing this round in its place, struck through.
function AgentInputs({ d, rounds, f }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null }) {
  const m = parseMessage(d.message ?? '');
  const sent = new Map(d.conditional.map((c) => [c.address, c.given]));
  const cell = d.round.cell !== null ? f?.cell(d.round.cell) : undefined;
  const title = (id: string) => {
    const c = f?.cell(id);
    return c ? `${id} ${c.title}` : id;
  };
  const rows = inputRows(cell, m.workFrom, sent);
  const theirs = new Set(rows.flatMap((x) => x.addresses.map((a) => a.address)));
  const others = d.conditional.filter((c) => !theirs.has(c.address));
  const alsoSent = others.filter((c) => c.given);
  const alsoHeld = others.filter((c) => !c.given);
  const itself = (cell?.inputs ?? []).filter((x) => x.from.every((s) => s === 'self'));
  return (
    <>
      {m.task !== null ? (
        <details className="section round-part">
          <summary>
            <span className="section-title">{t('round_task')}</span>{' '}
            <span className="muted">{t('round_task_same', { cell: d.round.cell ?? '' })}</span>
          </summary>
          <div className="section-body">
            <Markdown text={m.task} />
          </div>
        </details>
      ) : null}
      <section className="round-part">
        <h4 className="round-h">{t('round_inputs')}</h4>
        <ol className="round-inputs">
          {rows.map((x) => (
            <li
              key={x.no}
              value={x.no}
              className={x.addresses.length === 0 ? 'input-none' : undefined}
              title={x.addresses.length === 0 ? `${x.x.what}\n${t('input_nothing')}` : x.x.what}
            >
              <div className="input-row">
                <strong>{shortName(x.x)}</strong>
                {x.addresses.length === 1 ? (
                  <>
                    <span className="muted">{sourceWords(x.addresses[0].address, title)}</span>
                    <Chip sent={x.addresses[0].sent} />
                  </>
                ) : null}
              </div>
              {x.addresses.map((a) => (
                <div key={a.address} className="input-content">
                  {x.addresses.length > 1 ? (
                    <div className="input-row">
                      <span className="muted">{sourceWords(a.address, title)}</span> <Chip sent={a.sent} />
                    </div>
                  ) : null}
                  {a.sent ? (
                    <Held address={a.address} value={m.given.get(a.address) ?? ''} f={f} limits={d.limits} />
                  ) : (
                    <InSession address={a.address} d={d} rounds={rounds} f={f} />
                  )}
                </div>
              ))}
            </li>
          ))}
        </ol>
        {itself.length > 0 ? (
          <p className="muted">{t('round_itself', { names: itself.map(shortName).join(', ') })}</p>
        ) : null}
        {alsoSent.length > 0 ? (
          <div className="also-sent">
            <span className="muted">{t('round_also_sent')}</span>
            {alsoSent.map((c) => (
              <details key={c.address}>
                <summary>{addressWords(c.address)}</summary>
                <Markdown text={m.given.get(c.address) ?? ''} />
              </details>
            ))}
          </div>
        ) : null}
        {alsoHeld.length > 0 ? (
          <div className="also-sent">
            <span className="muted">{t('round_also_held')}</span>
            {alsoHeld.map((c) => (
              <InSession
                key={c.address}
                address={c.address}
                d={d}
                rounds={rounds}
                f={f}
                summary={addressWords(c.address)}
              />
            ))}
          </div>
        ) : null}
      </section>
    </>
  );
}

/// A file a tool kept, read when asked for.
function KeptFile({ file, path }: { file: string; path: string }) {
  const [text, setText] = useState<string | null>(null);
  const open = () => {
    if (text !== null) {
      setText(null);
      return;
    }
    const [dir, ...name] = file.split('/');
    getText(`/api/v1/files/${encodeURIComponent(dir)}/${encodeURIComponent(name.join('/'))}`).then(setText, (e: unknown) =>
      setText(String(e)),
    );
  };
  return (
    <div className="kept">
      <button className="link" onClick={open} title={path}>
        {file}
      </button>
      {text !== null ? <pre className="j-text file-text">{text}</pre> : null}
    </div>
  );
}

/// What a tool was asked, in words: a query's SQL as code, a commit as code, anything else by its fields.
function Asked({ args, f }: { args: JsonValue; f: Flow | null }) {
  const a = isObj(args) ? args : {};
  const tool = str(a.tool);
  const rest = Object.entries(a).filter(([k, v]) => k !== 'tool' && k !== 'sql' && !empty(v));
  return (
    <div className="tool-asked">
      <strong title={f?.compiled.tools.find((x) => x.name === tool)?.runs}>{tool}</strong>
      {typeof a.sql === 'string' ? <pre className="code-block">{a.sql}</pre> : null}
      {rest.map(([k, v]) => (
        <div key={k} className="labelled-field">
          <span className="decision-label">{label(k)}</span>
          {typeof v === 'string' && !v.includes(' ') ? <code>{v}</code> : <Content value={v} />}
        </div>
      ))}
    </div>
  );
}

/// A tool the engine ran: what it was asked, and what it gave, with the files it kept.
function Tool({ run, f }: { run: ToolRun; f: Flow | null }) {
  return (
    <div className="boxed tool-run">
      <Asked args={run.args} f={f} />
      <div className="muted">
        {run.attempt > 1 ? `${t('attempt', { n: run.attempt })} · ` : ''}
        {run.ended_at !== null && run.started_at !== null ? duration(run.ended_at - run.started_at) : ''}
      </div>
      {Array.isArray(run.result) && run.result.length === 2 && typeof run.result[1] === 'string' ? (
        <>
          <div className="muted">{t('tool_exit', { code: str(run.result[0]) })}</div>
          <pre className="j-text file-text">{run.result[1]}</pre>
        </>
      ) : (
        <Content value={run.result} />
      )}
      {run.files.map((x) => (
        <KeptFile key={x.file} file={x.file} path={x.path} />
      ))}
    </div>
  );
}

/// The tools a round asked for, and what they gave in the round that ran them, the next one of the work.
function ToolsAsked({ tools, work, next, f }: {
  tools: JsonValue[];
  work: number;
  next: RoundSummary | undefined;
  f: Flow | null;
}) {
  const ran = next !== undefined && next.cell !== null && f?.cell(next.cell)?.kind === 'tool';
  const detail = useResource<RoundDetail>(
    ran ? `round:${work}-${next.n}` : null,
    ran ? roundUrl(work, next.n) : null,
  );
  const runs = detail.data?.tool_runs ?? [];
  return (
    <div className="tools-asked">
      {runs.length > 0
        ? runs.map((run) => <Tool key={run.id} run={run} f={f} />)
        : tools.map((x, i) => (
            <div key={i} className="boxed tool-run">
              <Asked args={x} f={f} />
            </div>
          ))}
      {ran ? (
        <p className="muted">
          {t('tools_ran_in')}{' '}
          <RoundLink f={f} id={next.n}>
            {roundName(work, next.n)}
          </RoundLink>
        </p>
      ) : null}
    </div>
  );
}

/// A new acceptance, each condition marked new, changed or unchanged against the one it replaces, and the conditions
/// it leaves out.
function NewAcceptance({ value, before }: { value: JsonValue; before: JsonValue | null }) {
  const old = Array.isArray(before) ? before.filter(isObj) : [];
  const now = Array.isArray(value) ? value.filter(isObj) : [];
  if (old.length === 0 || now.length === 0) return <Acceptance value={value} met={new Map()} />;
  const same = (a: Obj, b: Obj) => str(a.condition) === str(b.condition) && str(a.by) === str(b.by) && str(a.criterion) === str(b.criterion);
  const dropped = old.filter((o) => !now.some((n) => str(n.id) === str(o.id)));
  return (
    <table className="list acceptance">
      <tbody>
        {now.map((n, i) => {
          const o = old.find((x) => str(x.id) === str(n.id));
          const mark = o === undefined ? 'new' : same(o, n) ? 'unchanged' : 'changed';
          return (
            <tr key={i} className={`acc-${mark}`}>
              <td className="nowrap">
                <code>{str(n.id)}</code>
              </td>
              <td>
                <Markdown text={str(n.condition)} />
                {mark === 'changed' && o !== undefined ? (
                  <details>
                    <summary className="muted">{t('acc_was')}</summary>
                    <Markdown text={str(o.condition)} />
                  </details>
                ) : null}
              </td>
              <td className="nowrap">
                <span className={`chip acc-chip-${mark}`}>
                  {t(mark === 'new' ? 'acc_new' : mark === 'changed' ? 'acc_changed' : 'acc_unchanged')}
                </span>
              </td>
            </tr>
          );
        })}
        {dropped.map((o, i) => (
          <tr key={`d${i}`} className="acc-removed">
            <td className="nowrap">
              <code>{str(o.id)}</code>
            </td>
            <td>
              <Markdown text={str(o.condition)} />
            </td>
            <td className="nowrap">
              <span className="chip acc-chip-removed">{t('acc_removed')}</span>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// A cell's own answer, each field drawn by its shape in the flow's formats where the page knows it, any other as
/// labelled text.
function Answer({ value, before }: { value: Obj; before: JsonValue | null }) {
  const chosen = str(value.chosen);
  return (
    <div className="shape">
      {Object.entries(value)
        .filter(([, v]) => !empty(v))
        .map(([k, v]) => {
          const name = label(k);
          switch (k) {
            case 'understanding':
              return <Shaped key={k} name={name} body={<Understanding value={v} />} />;
            case 'options':
              return <Shaped key={k} name={name} body={<Approaches value={v} chosen={chosen} />} />;
            case 'design':
              return <Shaped key={k} name={name} body={<Design value={v} after={null} />} />;
            case 'measure':
              return <Shaped key={k} name={name} body={<Measure value={v} />} />;
            case 'grounds':
              return <Shaped key={k} name={name} body={<Grounds value={v} />} />;
            case 'acceptance':
              return <Shaped key={k} name={name} body={<Acceptance value={v} met={new Map()} />} />;
            case 'new_acceptance':
              return <Shaped key={k} name={name} body={<NewAcceptance value={v} before={before} />} />;
            case 'why':
              // Why comes after what was decided, and is read on asking.
              return (
                <Fold key={k} title={<span className="decision-label">{name}</span>}>
                  <Labelled value={v} />
                </Fold>
              );
            case 'deviations':
              return (
                <Shaped
                  key={k}
                  name={name}
                  body={Array.isArray(v) ? v.map((x, i) => <Deviation key={i} value={x} />) : <Labelled value={v} />}
                />
              );
          }
          if (isObj(v)) return <Shaped key={k} name={name} body={<Answer value={v} before={before} />} />;
          return <Field key={k} label={name} value={v} />;
        })}
    </div>
  );
}

function Shaped({ name, body }: { name: string; body: ReactNode }) {
  return (
    <div className="field">
      <span className="decision-label">{name}</span>
      {body}
    </div>
  );
}

/// What an agent's round handed back: what it said it did, then the one form its return holds (its own answer, the
/// tools it asked for, its ask, a pause), then what it told the owner and its side reports.
function HandedBack({ d, rounds, f }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null }) {
  const out = isObj(d.output) ? d.output : null;
  if (out === null) return d.output !== null ? <Content value={d.output} /> : <p className="muted">{t('none')}</p>;
  const within = f?.g.within ?? 'outcome';
  const form = isObj(out[within]) ? (out[within] as Obj) : null;
  const at = rounds.findIndex((x) => x.n === d.round.n);
  const next = at >= 0 ? rounds[at + 1] : undefined;
  const told = Array.isArray(out.tell_owner) ? out.tell_owner.filter((x) => !empty(x)) : [];
  const side = Array.isArray(out.side_reports) ? out.side_reports.filter((x) => !empty(x)) : [];
  return (
    <>
      {typeof out.progress === 'string' && out.progress.trim() !== '' ? <Markdown text={out.progress} /> : null}
      {form !== null && Array.isArray(form.tools) ? (
        <ToolsAsked tools={form.tools} work={d.work} next={next} f={f} />
      ) : form !== null && 'ask' in form ? (
        d.asks.length > 0 ? (
          d.asks.map((a) => <AskView key={a.id} a={a} />)
        ) : (
          <Content value={form.ask} />
        )
      ) : form !== null && ('pause' in form || 'answers' in form || 'redirect' in form) ? (
        <Content value={form} />
      ) : form !== null ? (
        <Answer value={form} before={d.acceptance_before} />
      ) : null}
      {d.design_cards.map((c) => (
        <div key={c.id} className="field">
          <span className="decision-label">{t('round_design_card', { id: c.id, status: words(c.status) })}</span>
          <DesignCardView c={c} />
        </div>
      ))}
      {told.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('round_told_owner')}</span>
          {told.map((x, i) => (
            <Markdown key={i} text={str(x)} />
          ))}
        </div>
      ) : null}
      {side.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('round_side_reports')}</span>
          <Content value={side} />
        </div>
      ) : null}
    </>
  );
}

/// What a step that was not an agent's came to, without the progress the top already says.
function cameTo(output: RoundDetail['output']): RoundDetail['output'] {
  if (output === null || typeof output !== 'object' || Array.isArray(output) || typeof output.progress !== 'string') {
    return output;
  }
  const rest = Object.fromEntries(Object.entries(output).filter(([k]) => k !== 'progress'));
  return Object.keys(rest).length > 0 ? rest : null;
}

/// A raw text for the final check, folded: the message word for word, or a return exactly as handed in.
function Raw({ title, note = null, text }: { title: string; note?: string | null; text: string }) {
  return (
    <details className="section round-part">
      <summary>
        <span className="section-title">{title}</span>
      </summary>
      <div className="section-body">
        {note !== null ? <p className="muted">{note}</p> : null}
        <pre className="j-text prompt">{text}</pre>
      </div>
    </details>
  );
}

function Detail({ d, rounds }: { d: RoundDetail; rounds: RoundSummary[] }) {
  const f = useContext(FlowContext);
  const ran = d.message !== null;
  // What tools came to is what each of them gave, shown with them.
  const output = ran ? d.output : d.tool_runs.length > 0 ? null : cameTo(d.output);
  // For the final check, what was handed back as it was handed in: the agent's return (the one sent back, for a
  // return sent back), or what the tools gave, or what the step came to.
  const handed = d.tool_runs.length > 0 && !ran ? d.tool_runs.map((x) => x.result) : d.output;
  const raw = handed !== null ? JSON.stringify(handed, null, 2) : null;
  return (
    <div className="round">
      <Path d={d} rounds={rounds} f={f} />
      {ran ? <AgentInputs d={d} rounds={rounds} f={f} /> : null}
      {!ran && d.inputs.length > 0 ? (
        <details className="section round-part">
          <summary>
            <span className="section-title">{t('round_read')}</span>
          </summary>
          <div className="section-body">
            {d.inputs.map((x, i) => (
              <div key={i} className="field">
                <span className="decision-label">
                  {x.what}
                  {x.from.length > 0 ? <span className="muted"> · {x.from.map(srcText).join('; ')}</span> : null}
                </span>
                <Content value={x.value} />
              </div>
            ))}
          </div>
        </details>
      ) : null}
      {d.tool_runs.length > 0 ? (
        <section className="round-part">
          <h4 className="round-h">{t('round_tools', { n: d.tool_runs.length })}</h4>
          {d.tool_runs.map((run) => (
            <Tool key={run.id} run={run} f={f} />
          ))}
        </section>
      ) : null}
      {ran || output !== null ? (
        <section className="round-part">
          <h4 className="round-h">
            {ran ? t('round_returned') : t('round_came_to')}
            {d.round.transition !== null ? (
              <>
                {' '}
                <RowChip row={d.round.transition} />
              </>
            ) : null}
          </h4>
          {ran ? <HandedBack d={d} rounds={rounds} f={f} /> : <Content value={output} />}
        </section>
      ) : null}
      {!ran
        ? d.asks.map((a) => (
            <section key={a.id} className="round-part">
              <h4 className="round-h">{t('round_ask', { id: a.id })}</h4>
              <AskView a={a} />
            </section>
          ))
        : null}
      {ran ? <Raw title={t('message')} text={d.message ?? ''} /> : null}
      {raw !== null ? (
        <Raw
          title={ran ? t('round_raw_return') : d.tool_runs.length > 0 ? t('round_raw_tools') : t('round_raw_came_to')}
          note={d.round.outcome === 'sent_back' ? t('round_raw_sent_back', { why: d.round.why ?? '' }) : null}
          text={raw}
        />
      ) : null}
    </div>
  );
}

/// Where the interface gives a round in full: by its work and its number in the work.
function roundUrl(work: number, n: number): string {
  return `/api/v1/works/${work}/rounds/${n}`;
}

/// A round of a work, by its number in the work, with the work's rounds around it.
export function RoundView({ work, n, rounds = [] }: { work: number; n: number; rounds?: RoundSummary[] }) {
  const round = useResource<RoundDetail>(`round:${work}-${n}`, roundUrl(work, n));
  return <Loaded r={round}>{(d) => <Detail d={d} rounds={rounds} />}</Loaded>;
}
