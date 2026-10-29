// A round, as its panel shows it, by one rule: what the agent got or gave is inside a box, exactly, and everything the
// page adds is outside the boxes, in the page's own style. Under the panel's title (the round and its cell, a link to
// the cell on the chart): one small line of run facts and, for an agent's round, one of where its tokens and cost
// went. Then its input: the conditional inputs the message gave or referred to, one row each with its address and
// what it is, those the session already held, and so not sent, greyed and opening to what the session held; and the
// message as sent, word for word, in one box, every heading and line as it is, a long part folded under a label that
// gives its size and its share of the message, its first lines showing. Then its output: the return as handed back in
// one box, each key on its own line with what the cell's return format says it is beside it, and its value in a box
// of its own under it, laid out by the cell's compiled return schema alone (no code for any cell): nested keys
// indented, lists numbered, in a list of objects each key's note once at its head and the items as their values, a
// long table of rows folded, an empty list said so, nothing hidden, renamed, merged or added; the raw JSON folded at
// the bottom. A return sent back has why above its box. A round no agent ran shows what its step read and what it came
// to by the same rule; for the tools, what they were asked and what they returned.

import { type ReactNode, useContext, useState } from 'react';

import type { CompiledCell, JsonValue, RoundDetail, RoundSummary, ToolRun } from '../api';
import { Loaded } from '../components';
import { numbered } from '../flow/CellPanel';
import { type Flow, FlowContext } from '../flow/context';
import { cost, duration, time } from '../format';
import { getJson, getText } from '../http';
import { isObj, type Obj, words } from '../labelled';
import { useResource } from '../live';
import { RoundLink } from '../parts';
import {
  brief,
  chars,
  FOLD_AT,
  foldLabel,
  type HeadKey,
  messageParts,
  placeSchema,
  preview,
  ROWS_FOLD_AT,
  ROWS_PREVIEW,
  rowsLabel,
  shape,
  type ShapeOptions,
  type Shown,
  type ShownField,
  spent,
} from '../round';
import { t } from '../text';
import { AskView } from './Decide';

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

/// A model's name as people say it: `claude-opus-5-5` is Opus 5.5; any other name as it is.
export function modelName(model: string): string {
  const m = /^claude-([a-z]+)-(\d+)-(\d+)$/.exec(model);
  return m ? `${m[1].charAt(0).toUpperCase()}${m[1].slice(1)} ${m[2]}.${m[3]}` : model;
}

/// The value at a path of a return, as an address names it (`end.test_changes[added]`): a branch keeps only what is of
/// that branch, and the values at a path inside a list are a list.
export function atPath(value: JsonValue | null | undefined, path: string): JsonValue | undefined {
  let v: JsonValue | undefined = value ?? undefined;
  for (const seg of path.split('.')) {
    const hit = /^([^[]+)(?:\[(.+)\])?$/.exec(seg);
    if (hit === null || v === undefined) return undefined;
    const [, name, fixed] = hit;
    const of = (x: JsonValue): JsonValue | undefined => (isObj(x) ? x[name] : undefined);
    v = Array.isArray(v) ? v.map(of).filter((x): x is JsonValue => x !== undefined) : of(v);
    if (fixed !== undefined && v !== undefined) {
      const mine = (x: JsonValue) => isObj(x) && Object.values(x).some((y) => y === fixed);
      v = Array.isArray(v) ? v.filter(mine) : mine(v) ? v : undefined;
    }
  }
  return v;
}

/// The description a schema gives, if any.
const said = (s: Obj | null): string | null => (s !== null && typeof s.description === 'string' ? s.description : null);

/// What a conditional input is, in words, by its address: for an input of the cell, its name, or the description of
/// the place of the return it is in the return format of the cell that gave it, or what the cell says the input is;
/// for a return format, a rule, the worktree and the like, what that is.
export function inputWords(address: string, cell: CompiledCell | undefined, f: Flow | null, workFrom: string[]): string {
  const colon = address.indexOf(':');
  if (colon > 0 && !address.startsWith('R')) {
    const kind = address.slice(0, colon);
    const rest = address.slice(colon + 1);
    switch (kind) {
      case 'format':
        return rest === 'common' ? t('addr_common_format') : rest === cell?.id ? t('addr_own_format') : t('addr_cell_format', { cell: rest });
      case 'inputs':
        return t('addr_cell_inputs', { cell: rest });
      case 'rule': {
        const rule = f?.g.rules.find((r) => r.name === rest);
        return rule !== undefined ? brief(rule.desc) : t('addr_rule', { name: rest });
      }
      case 'work':
        return t('addr_work');
      case 'item':
        return t('addr_item');
      case 'config': {
        const x = cell?.inputs.find((i) => i.from.some((s) => typeof s === 'object' && !Array.isArray(s) && 'config' in s && s.config === rest));
        return x !== undefined ? (x.name ?? brief(x.what)) : t('addr_setting', { name: words(rest) });
      }
      case 'ij':
        return t('addr_interjection');
      case 'design_card':
        return t('addr_design_card', { id: rest });
      case 'ask':
        return t('addr_ask', { id: rest });
    }
    return address;
  }
  // The cell's own input the list to work from gives this address for.
  let no: number | null = null;
  for (const [k, xs] of addressesByNumber(workFrom)) if (xs.includes(address)) no = k;
  const x = cell !== undefined && no !== null ? numbered(cell).find((y) => y.no === no)?.x : undefined;
  const [, from = '', path = ''] = address.split('·');
  if (x?.name) return x.name;
  if (from === 'engine') return x !== undefined ? brief(x.what) : brief(f?.compiled.engine.inputs[path] ?? words(path));
  const place = said(placeSchema(f?.cell(from)?.schema, path));
  if (place !== null) return brief(place);
  return x !== undefined ? brief(x.what) : address;
}

/// The lines of a text, exactly, each a line of its own, an empty one keeping its height.
function Lines({ text }: { text: string }) {
  return (
    <>
      {text.split('\n').map((l, i) => (
        <div key={i} className="rp-line">
          {l}
        </div>
      ))}
    </>
  );
}

/// A part of the message between two headings: as it is, or, when long, folded under a label in the page's words
/// that gives its size and its share of the message, its first lines showing.
function MessageText({ text, total }: { text: string; total: number }) {
  const [open, setOpen] = useState(false);
  const n = chars(text);
  if (n <= FOLD_AT) return <Lines text={text} />;
  return (
    <div className="rp-fold">
      <button type="button" className="rp-fold-label" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? '▾' : '▸'} {foldLabel(n, total, open)}
      </button>
      {open ? (
        <Lines text={text} />
      ) : (
        <div className="rp-preview">
          <Lines text={preview(text)} />
        </div>
      )}
    </div>
  );
}

/// A message as sent, word for word: every heading and line as it is, in order. The task is never folded, since it is
/// what the agent was asked.
function Message({ text }: { text: string }) {
  const total = chars(text);
  const parts = messageParts(text);
  return (
    <div className="rp-msg">
      {parts.map((p, i) => {
        if ('heading' in p) {
          return (
            <div key={i} className="rp-line rp-mh">
              {p.heading}
            </div>
          );
        }
        const under = parts[i - 1];
        if (under !== undefined && 'heading' in under && under.heading === '## Task') return <Lines key={i} text={p.text} />;
        return <MessageText key={i} text={p.text} total={total} />;
      })}
    </div>
  );
}

/// What the agent, the tools or the engine's step got or gave, in a box under its label; open at first, a click on
/// the label folds it down to the label, and another opens it again.
function Box({ label, kind, children }: { label: string; kind: 'in' | 'out'; children: ReactNode }) {
  const [open, setOpen] = useState(true);
  return (
    <div className={`rp-box rp-box-${kind}${open ? '' : ' rp-box-folded'}`}>
      <button type="button" className="rp-box-label" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? '▾' : '▸'} {label}
      </button>
      {open ? <div className="rp-box-body">{children}</div> : null}
    </div>
  );
}

/// A text value as it is; one that is only a link, a link.
function TextValue({ text }: { text: string }) {
  if (/^https?:\/\/\S+$/.test(text)) {
    return (
      <a href={text} target="_blank" rel="noreferrer">
        {text}
      </a>
    );
  }
  return <>{text}</>;
}

/// Rows a query found, as a table in a small box of its own; a long one folded under a label that says how many rows
/// it has, its first rows showing.
function Rows({ columns, rows }: { columns: string[]; rows: Array<Array<string | null>> }) {
  const [open, setOpen] = useState(false);
  const long = rows.length > ROWS_FOLD_AT;
  const table = (shown: Array<Array<string | null>>) => (
    <div className="rp-val rp-table-wrap">
      <table className="rp-table">
        <thead>
          <tr>
            <th />
            {columns.map((c) => (
              <th key={c}>{c}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {shown.map((row, i) => (
            <tr key={i}>
              <td className="rp-note">{i + 1}</td>
              {row.map((x, j) => (
                <td key={j}>{x}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
  if (!long) return table(rows);
  return (
    <div className="rp-fold">
      <button type="button" className="rp-fold-label" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? '▾' : '▸'} {rowsLabel(rows.length, open)}
      </button>
      {open ? table(rows) : <div className="rp-preview">{table(rows.slice(0, ROWS_PREVIEW))}</div>}
    </div>
  );
}

/// The head of a list of objects: each key with the note its items share, once, the keys under it indented; outside
/// the values, as every note is.
function ListHead({ keys }: { keys: HeadKey[] }) {
  return (
    <div className="rp-head">
      <div className="rp-note">{t('shape_head')}</div>
      <HeadKeys keys={keys} />
    </div>
  );
}

function HeadKeys({ keys }: { keys: HeadKey[] }) {
  return (
    <>
      {keys.map((k) => (
        <div key={k.key} className="rp-head-key">
          <div className="rp-kh">
            <code className="rp-key">{k.key}</code>
            {k.note !== null ? (
              <span className="rp-note" title={k.full ?? undefined}>
                {k.note}
              </span>
            ) : null}
          </div>
          {k.under.length > 0 ? (
            <div className="rp-ind">
              <HeadKeys keys={k.under} />
            </div>
          ) : null}
        </div>
      ))}
    </>
  );
}

/// A value that is not an object, in a small box of its own: a text, a number, a boolean or null as it is, an empty
/// list or object said so, a list as numbered rows, a list of objects under the head that gives its keys' notes.
function ValueBox({ v }: { v: Shown }) {
  switch (v.kind) {
    case 'text':
    case 'plain':
      return (
        <div className="rp-val">
          <div className="rp-vt">
            <TextValue text={v.text} />
          </div>
        </div>
      );
    case 'empty':
      return (
        <div className="rp-val">
          <span className="rp-vt">{v.text}</span> <span className="rp-note">{t('rp_empty')}</span>
        </div>
      );
    case 'fields':
      return (
        <div className="rp-ind">
          <Fields fields={v.fields} />
        </div>
      );
    case 'table':
      return <Rows columns={v.columns} rows={v.rows} />;
    case 'list':
      if (v.items.every((x) => x.value.kind === 'fields')) {
        return (
          <div className="rp-ind">
            {v.head !== undefined && v.head.length > 0 ? <ListHead keys={v.head} /> : null}
            {v.items.map((x, i) => (
              <div key={i} className="rp-item">
                <div className="rp-note">{t('rp_item', { n: i + 1 })}</div>
                {x.value.kind === 'fields' ? <Fields fields={x.value.fields} /> : null}
              </div>
            ))}
          </div>
        );
      }
      return (
        <div className="rp-val">
          {v.items.map((x, i) => (
            <div key={i} className="rp-li">
              <span className="rp-note">{i + 1}</span>
              <div>
                {x.value.kind === 'text' || x.value.kind === 'plain' ? (
                  <span className="rp-vt">
                    <TextValue text={x.value.text} />
                  </span>
                ) : (
                  <ValueBox v={x.value} />
                )}
                {x.note !== null ? <span className="rp-note"> {x.note}</span> : null}
              </div>
            </div>
          ))}
        </div>
      );
  }
}

/// An object's keys, each on its own line with what its schema says it is beside it, unless the head of the list it
/// is in says so, and how many items a list holds, its value under it: nested keys indented, any other value in a box
/// of its own.
function Fields({ fields }: { fields: ShownField[] }) {
  return (
    <>
      {fields.map((x) => (
        <div key={x.key} className="rp-field">
          <div className="rp-kh">
            <code className="rp-key">{x.key}</code>
            {x.note !== null || x.count !== null ? (
              <span className="rp-note" title={x.full ?? undefined}>
                {[x.note, x.count].filter((s) => s !== null).join(' ')}
              </span>
            ) : null}
          </div>
          <ValueBox v={x.value} />
        </div>
      ))}
    </>
  );
}

/// A value laid out by its schema, the whole of what a box holds.
function Value({ v }: { v: Shown }) {
  return v.kind === 'fields' ? <Fields fields={v.fields} /> : <ValueBox v={v} />;
}

/// A cell, as a link that brings it into view on the chart and opens it.
function CellLink({ f, id }: { f: Flow | null; id: string }) {
  const c = f?.cell(id);
  if (f === null) return <span>{c ? `${id} ${c.title}` : id}</span>;
  return (
    <button type="button" className="link" onClick={() => f.open({ kind: 'cell', cell: id })}>
      <strong>{id}</strong> {c?.title ?? ''}
    </button>
  );
}

/// A round of the work, as a round link that opens it on the chart, or on the work's page where no chart is.
function Round({ f, work, n }: { f: Flow | null; work: number; n: number }) {
  return <RoundLink work={work} round={n} onRound={f !== null ? (_, r) => f.open({ kind: 'round', round: r }) : undefined} />;
}

/// Why a round started a new session, in a few words; none for a round that went on in its line's session.
function newSession(r: RoundSummary): string | null {
  switch (r.new_session) {
    case 'cell':
      return t('run_new_session_cell', { cell: r.cell ?? '' });
    case 'limit':
      return t('run_new_session_limit');
    case 'model':
      return t('run_new_session_model');
    case 'owner':
      return t('run_new_session_owner');
    case 'line':
      return r.line !== null && r.line === r.cell ? t('run_new_session_own') : t('run_new_session_line');
    default:
      return null;
  }
}

/// Who did the round's work and how it ran, in a few words: for an agent its line, level, model, effort, whether it
/// started a new session and why, or continued its line's session, its time and cost; else the tools, the engine's
/// step or the owner.
function runWords(d: RoundDetail, cell: CompiledCell | undefined): string {
  const r = d.round;
  const took = r.ended_at !== null ? duration(r.ended_at - r.started_at) : t('rp_running_for', { time: duration(Date.now() - r.started_at) });
  if (d.message !== null) {
    const call = isObj(d.call) ? d.call : {};
    const session = isObj(call.session) ? call.session : {};
    return [
      r.line,
      d.level !== null ? t('run_level', { level: d.level }) : null,
      r.model !== null ? modelName(r.model) : null,
      d.effort !== null ? t('run_effort', { effort: d.effort }) : null,
      newSession(r) ?? ('started' in session ? t('run_started') : 'resumed' in session ? t('run_continued') : null),
      r.compactions > 0 ? t('run_compacted') : null,
      took,
      r.cost_usd !== null ? cost(r.cost_usd) : null,
    ]
      .filter((x) => x !== null && x !== '')
      .join(' · ');
  }
  if (r.not_asked !== null) return t('run_not_asked', { need: words(r.not_asked) });
  if (cell?.kind === 'tool') return [t('round_by_tools'), t('rp_tool_runs', { n: d.tool_runs.length }), took].join(' · ');
  if (cell?.kind === 'owner') return [t('round_by_owner'), took].join(' · ');
  return [t('round_by_engine'), cell?.step ?? null, took].filter((x) => x !== null).join(' · ');
}

/// Where an agent's round spent its tokens and its cost, one short line, each part's tokens on hovering.
function SpentLine({ r }: { r: RoundSummary }) {
  if (r.tokens === null) return null;
  const s = spent(r.tokens, r.model);
  return (
    <p className="rp-facts rp-spent">
      {s.parts.map((p, i) => (
        <span key={i} title={p.title ?? undefined}>
          {i > 0 ? ' · ' : ''}
          {p.share !== undefined ? <i className={`spent-dot spent-${p.share}`} /> : null}
          {p.text}
        </span>
      ))}
    </p>
  );
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
    const output = x.output;
    const form = isObj(output) ? output[within] : undefined;
    const out = isObj(output) && isObj(form) ? { ...output, ...form } : output;
    return atPath(out, m[4]) ?? null;
  }
  return null;
}

/// One conditional input of the message: its address and what it is, in bold when this round sent it; one the session
/// already held, and so was not sent, greyed, opening to what the session held, outside the box since this round did
/// not send it.
function InputRow({ address, given, hash, words: what, d, rounds, f }: {
  address: string;
  given: boolean;
  hash: string;
  words: string;
  d: RoundDetail;
  rounds: RoundSummary[];
  f: Flow | null;
}) {
  const [open, setOpen] = useState(false);
  const [value, setValue] = useState<JsonValue | null | undefined>(undefined);
  const row = (
    <>
      <code className="rp-addr">{address}</code>
      <span>{what}</span>
    </>
  );
  if (given) return <div className="rp-input rp-sent">{row}</div>;
  const toggle = () => {
    setOpen(!open);
    if (value === undefined) findHeld(address, hash, d, rounds, f?.g.within ?? 'outcome').then(setValue, () => setValue(null));
  };
  const [, from = '', path = ''] = address.split('·');
  return (
    <>
      <button type="button" className="rp-input rp-held" aria-expanded={open} onClick={toggle} title={t('rp_held_hint')}>
        {row}
      </button>
      {open ? (
        <div className="rp-held-body">
          <div className="rp-note">{t('rp_held_label')}</div>
          {value === undefined ? (
            <p className="rp-note">{t('loading')}</p>
          ) : value === null ? (
            <p className="rp-note">{t('input_not_found')}</p>
          ) : typeof value === 'string' ? (
            <div className="rp-msg">
              <Lines text={value} />
            </div>
          ) : (
            <Value v={shape(value, from !== 'engine' ? placeSchema(f?.cell(from)?.schema, path) : null)} />
          )}
        </div>
      ) : null}
    </>
  );
}

/// The lines of the message's list of inputs to work from.
function workFromOf(message: string): string[] {
  return parseMessage(message).workFrom;
}

/// An agent's input: the conditional inputs listed above the box, then the message as sent, word for word.
function AgentInput({ d, rounds, f, cell }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null; cell: CompiledCell | undefined }) {
  const message = d.message ?? '';
  const from = workFromOf(message);
  const sent = d.conditional.filter((c) => c.given).length;
  return (
    <section className="rp-sec">
      <h4 className="rp-h">{t('rp_input')}</h4>
      {d.conditional.length > 0 ? (
        <>
          <p className="rp-note">{t('rp_inputs_count', { sent, held: d.conditional.length - sent })}</p>
          <div className="rp-inputs">
            {d.conditional.map((c) => (
              <InputRow
                key={c.address}
                address={c.address}
                given={c.given}
                hash={c.hash}
                words={inputWords(c.address, cell, f, from)}
                d={d}
                rounds={rounds}
                f={f}
              />
            ))}
          </div>
        </>
      ) : null}
      <Box label={t('rp_box_sent')} kind="in">
        <Message text={message} />
      </Box>
    </section>
  );
}

/// The schema an input of a step that is not an agent's is read by: that of the place of the return it comes from,
/// in the return format of the cell that gave it (the cell the work came from, for `@caller` and `@from`); none for
/// what the engine, the configuration or the database gives.
function inputSchema(x: RoundDetail['inputs'][number], before: RoundSummary | undefined, f: Flow | null): Obj | null {
  const src = x.from[0];
  if (!Array.isArray(src)) return null;
  const [at, path] = src;
  const id = at.startsWith('@') ? before?.cell : at;
  return id != null ? placeSchema(f?.cell(id)?.schema, path) : null;
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
      <button type="button" className="link" onClick={open} title={path}>
        {file}
      </button>
      {text !== null ? <pre className="j-text file-text">{text}</pre> : null}
    </div>
  );
}

/// The tools a step that is not a tool cell ran, each as it was asked and what it returned.
function StepTools({ runs }: { runs: ToolRun[] }) {
  return (
    <Box label={t('rp_box_step_tools')} kind="out">
      {runs.map((run) => (
        <div key={run.id} className="rp-item">
          <div className="rp-note">
            {[
              run.tool,
              run.attempt > 1 ? t('attempt', { n: run.attempt }) : null,
              run.ended_at !== null && run.started_at !== null ? duration(run.ended_at - run.started_at) : null,
            ]
              .filter((x) => x !== null)
              .join(' · ')}
          </div>
          <Value v={shape({ args: run.args, result: run.result }, null)} />
        </div>
      ))}
    </Box>
  );
}

/// A raw JSON for the final check, folded.
function Raw({ title, value }: { title: string; value: JsonValue }) {
  return (
    <details className="rp-raw">
      <summary className="rp-note">{title}</summary>
      <pre className="j-text file-text">{JSON.stringify(value, null, 2)}</pre>
    </details>
  );
}

function Detail({ d, rounds }: { d: RoundDetail; rounds: RoundSummary[] }) {
  const f = useContext(FlowContext);
  const r = d.round;
  const cell = r.cell !== null ? f?.cell(r.cell) : undefined;
  const agent = d.message !== null;
  const tool = cell?.kind === 'tool';
  const at = rounds.findIndex((x) => x.n === r.n);
  const before = at > 0 ? rounds[at - 1] : undefined;
  const after = at >= 0 ? rounds[at + 1] : undefined;
  const options: ShapeOptions = {
    within: f?.g.within,
    form: (i) => {
      const form = cell?.forms?.[i];
      if (form === undefined || cell === undefined) return null;
      return form.own ? t('rp_form_own', { cell: cell.id }) : form.field != null ? t('rp_form_common', { form: form.field }) : null;
    },
  };
  const outLabel = agent
    ? t('rp_box_returned')
    : r.not_asked !== null
      ? t('rp_box_not_asked')
      : tool
        ? t('rp_box_tools_returned')
        : t('rp_box_came_to');
  const files = d.tool_runs.flatMap((x) => x.files);
  return (
    <div className="rp">
      <p className="rp-facts" title={r.ended_at !== null ? t('happened_at', { time: time(r.ended_at) }) : undefined}>
        {runWords(d, cell)}
      </p>
      {agent ? <SpentLine r={r} /> : null}
      {agent ? <AgentInput d={d} rounds={rounds} f={f} cell={cell} /> : null}
      {!agent && d.inputs.length > 0 ? (
        <section className="rp-sec">
          <h4 className="rp-h">{t('rp_input')}</h4>
          {tool && before !== undefined ? (
            <p className="rp-note">
              {t('rp_asked_by')} <Round f={f} work={d.work} n={before.n} />
              {before.cell !== null ? (
                <>
                  {' '}
                  (<CellLink f={f} id={before.cell} />)
                </>
              ) : null}
            </p>
          ) : null}
          <Box label={tool ? t('rp_box_tools_given') : t('rp_box_step_read')} kind="in">
            {d.inputs.map((x, i) => (
              <div key={i} className="rp-field">
                <div className="rp-kh">
                  <span className="rp-note" title={brief(x.what) !== x.what ? x.what : undefined}>
                    {brief(x.what)}
                  </span>
                </div>
                <ValueBox v={shape(x.value, inputSchema(x, before, f))} />
              </div>
            ))}
          </Box>
        </section>
      ) : null}
      <section className="rp-sec">
        <h4 className="rp-h">{t('rp_output')}</h4>
        {d.output !== null ? <p className="rp-note">{t('rp_output_note')}</p> : null}
        {r.outcome === 'sent_back' ? <p className="rp-note rp-bad">{t('rp_sent_back', { why: r.why ?? '' })}</p> : null}
        {r.outcome === 'failed' || r.outcome === 'interrupted' ? (
          <p className="rp-note rp-bad">{t('rp_no_return', { why: r.why ?? '' })}</p>
        ) : null}
        {d.output !== null ? (
          <Box label={outLabel} kind="out">
            <Value v={shape(d.output, cell?.schema, options)} />
          </Box>
        ) : r.outcome === 'running' ? (
          <p className="rp-note">{t('rp_running')}</p>
        ) : null}
        {!tool && d.tool_runs.length > 0 ? <StepTools runs={d.tool_runs} /> : null}
        {files.length > 0 ? (
          <div className="rp-files">
            <span className="rp-note">{t('rp_files')}</span>
            {files.map((x) => (
              <KeptFile key={x.file} file={x.file} path={x.path} />
            ))}
          </div>
        ) : null}
        {agent && after !== undefined && after.cell !== null && f?.cell(after.cell)?.kind === 'tool' ? (
          <p className="rp-note">
            {t('rp_tools_ran_in')} <Round f={f} work={d.work} n={after.n} />
          </p>
        ) : null}
        {!agent ? d.asks.map((a) => <AskView key={a.id} a={a} />) : null}
        {d.output !== null ? <Raw title={t('rp_raw')} value={d.output} /> : null}
      </section>
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
