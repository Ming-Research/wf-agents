// A round, as its panel shows it: what the round was and why it ran, what the agent was given, what it handed back and
// what it cost, each text in its voice's look (voices.tsx), the page's words never inside the other two. Under the
// panel's title (the round and its cell), an overview of four rows: Ran, Why, Led to, Cost. Then two sections, folded
// at first and kept open or folded from round to round as the owner leaves them: what it was given (the inputs new
// this round, those already in its session, folded under their count and opening to what the session held, and the
// message as sent, exactly, in one sheet, its long parts folded with their size and share), and what it handed back
// (the form the return holds, then each field under its title with its code after it, its value in the box of what an
// agent handed back, a list of objects as rows of one box, and the return as JSON folded at the bottom). A round of the
// tools shows each tool's question and its answer together; a round of an engine's step what it read and what it came
// to in the page's own words, a box only for what an agent handed back.

import { Flash, Label } from '@primer/react';
import { type ReactNode, useContext, useEffect, useState } from 'react';

import type { CompiledCell, JsonValue, RoundDetail, RoundSummary, ToolRun } from '../api';
import { Loaded } from '../components';
import { inputName, numbered } from '../flow/CellGiven';
import { type Flow, FlowContext } from '../flow/context';
import { inline } from '../flow/words';
import { cost, duration, time } from '../format';
import { getJson, getText } from '../http';
import { empty, isObj, type Obj, words } from '../labelled';
import { useResource } from '../live';
import { Markdown } from '../markdown';
import { LinkButton, RoundLink } from '../parts';
import { brief, chars, messageParts, placeSchema, shape, type ShapeOptions, shareWords, type Shown, type ShownField, spent, titleOf } from '../round';
import { t } from '../text';
import { counted, FoldPart, InnerName, joined, Long, Name, Named, Overview, PanelSection, Part, Row, Rows, Said, Sheet } from '../voices';
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

/// What a conditional input is, in words, by its address: for an input of the cell, its name; for a return format, a
/// rule, the worktree and the like, what that is.
export function inputWords(address: string, cell: CompiledCell | undefined, f: Flow | null, workFrom: string[]): string {
  const w = addressWords(address, cell, f, workFrom);
  return w.charAt(0).toUpperCase() + w.slice(1);
}

function addressWords(address: string, cell: CompiledCell | undefined, f: Flow | null, workFrom: string[]): string {
  const colon = address.indexOf(':');
  if (colon > 0 && !address.startsWith('R')) {
    const kind = address.slice(0, colon);
    const rest = address.slice(colon + 1);
    switch (kind) {
      case 'format':
        return rest === 'common' ? t('given_common_format') : rest === cell?.id ? t('given_cell_format') : t('addr_cell_format', { cell: rest });
      case 'inputs':
        return t('given_cell_inputs', { cell: rest });
      case 'rule':
        return t('given_rule', { name: rest });
      case 'work':
        return t('given_workspace');
      case 'item':
        return t('addr_item');
      case 'config': {
        const x = cell?.inputs.find((i) => i.from.some((s) => typeof s === 'object' && !Array.isArray(s) && 'config' in s && s.config === rest));
        return x !== undefined ? inputName(x) : t('addr_setting', { name: words(rest) });
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
  if (x !== undefined) return inputName(x);
  const [, from = '', path = ''] = address.split('·');
  if (from === 'engine') return brief(f?.compiled.engine.inputs[path] ?? words(path));
  return words(path) || address;
}

/// The kept state of the round panel's sections: which the owner left open, from round to round.
let roundOpen = new Set<string>();

/// A text an agent handed back, as markdown reads it, folded past 1,000 characters.
function SaidText({ text }: { text: string }) {
  return <Long text={text}>{(x) => <Markdown text={x} />}</Long>;
}

/// A list of rows in one box, three shown of more and a last row that shows them all.
function SaidRows({ rows }: { rows: ReactNode[] }) {
  const [all, setAll] = useState(false);
  const shown = all || rows.length <= 3 ? rows : rows.slice(0, 3);
  return (
    <Said list>
      {shown.map((r, i) => (
        <div key={i} className="said-row">
          <span className="pp-no">{i + 1}</span>
          <div className="said-row-body">{r}</div>
        </div>
      ))}
      {rows.length > 3 ? (
        <div className="said-more">
          <LinkButton className="long-more" onClick={() => setAll(!all)}>
            {all ? t('long_less') : t('rows_more', { n: rows.length })}
          </LinkButton>
        </div>
      ) : null}
    </Said>
  );
}

/// A field's name inside a box: its title, or its key where the schema does not describe it.
const fieldName = (f: ShownField) => f.title ?? f.key;

/// An object's values inside a row of a box: the kinds as Labels and the short values by their names on its first
/// line, its first text as the row's text, and each other value under its name.
function RowValues({ fields }: { fields: ShownField[] }) {
  const first = fields.filter((x) => x.value.kind === 'fixed' || x.value.kind === 'plain');
  const text = fields.find((x) => x.value.kind === 'text');
  const rest = fields.filter((x) => !first.includes(x) && x !== text);
  return (
    <>
      {first.length > 0 ? (
        <div className="said-row-line">
          {first.map((x) =>
            x.value.kind === 'fixed' ? (
              <Label key={x.key} variant="secondary">
                {x.value.text}
              </Label>
            ) : (
              <span key={x.key}>
                <InnerName>{fieldName(x)}</InnerName> <Name>{x.value.kind === 'plain' ? x.value.text : ''}</Name>
              </span>
            ),
          )}
        </div>
      ) : null}
      {text !== undefined && text.value.kind === 'text' ? <SaidText text={text.value.text} /> : null}
      {rest.map((x) => (
        <div key={x.key} className="said-sub">
          <InnerName>{fieldName(x)}</InnerName>
          <Inside v={x.value} />
        </div>
      ))}
    </>
  );
}

/// A value inside a box, with no box of its own.
function Inside({ v }: { v: Shown }) {
  switch (v.kind) {
    case 'text':
      return <SaidText text={v.text} />;
    case 'plain':
      return <Name>{v.text}</Name>;
    case 'fixed':
      return <Label variant="secondary">{v.text}</Label>;
    case 'empty':
      return <InnerName>{t('rp_empty')}</InnerName>;
    case 'fields':
      return <RowValues fields={v.fields} />;
    case 'list':
      return (
        <>
          {v.items.map((x, i) => (
            <div key={i} className="said-row-line">
              <span className="pp-no">{i + 1}</span>
              <div className="said-row-body">
                <Inside v={x} />
              </div>
            </div>
          ))}
        </>
      );
  }
}

/// A value an agent handed back, in its box: a text as markdown, a kind as a Label, a list as rows of one box.
function Value({ v, level }: { v: Shown; level: number }) {
  switch (v.kind) {
    case 'text':
      return (
        <Said>
          <SaidText text={v.text} />
        </Said>
      );
    case 'plain':
      return <Said mono>{v.text}</Said>;
    case 'fixed':
      return (
        <Said>
          <span>
            <Label variant="secondary">{v.text}</Label>
          </span>
        </Said>
      );
    case 'empty':
      return (
        <Said>
          <InnerName>{t('rp_empty')}</InnerName>
        </Said>
      );
    case 'fields':
      return (
        <div className="pp-fields">
          {v.fields.map((x) => (
            <FieldValue key={x.key} f={x} level={level + 1} />
          ))}
        </div>
      );
    case 'list':
      return (
        <SaidRows
          rows={v.items.map((x, i) =>
            x.kind === 'fields' ? <RowValues key={i} fields={x.fields} /> : <Inside key={i} v={x} />,
          )}
        />
      );
  }
}

/// A field of what was handed back: its title as a sub-heading, its code after it, its count, and its value.
function FieldValue({ f, level }: { f: ShownField; level: number }) {
  const as = `h${Math.min(level, 6)}` as 'h3' | 'h4' | 'h5' | 'h6';
  const n = f.value.kind === 'list' ? f.value.items.length : undefined;
  return (
    <div className="pp-part">
      <Named title={f.title ?? <Name>{f.key}</Name>} code={f.title !== null ? f.key : undefined} n={n} as={as} />
      <Value v={f.value} level={level} />
    </div>
  );
}

/// A cell, as a link that brings it into view on the chart and opens it.
function CellLink({ f, id }: { f: Flow | null; id: string }) {
  const c = f?.cell(id);
  const words = (
    <>
      <span className="cell-id">{id}</span> {c?.title ?? ''}
    </>
  );
  if (f === null) return <span>{words}</span>;
  return <LinkButton onClick={() => f.open({ kind: 'cell', cell: id })}>{words}</LinkButton>;
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

/// Who did the round's work, in a few words: for an agent its line, model, level and effort, and whether it went on in
/// its session or started one and why; else the tools, the engine's step or the owner.
function ranWords(d: RoundDetail, cell: CompiledCell | undefined): string {
  const r = d.round;
  if (d.message !== null) {
    const call = isObj(d.call) ? d.call : {};
    const session = isObj(call.session) ? call.session : {};
    return [
      r.line,
      r.model !== null ? modelName(r.model) : null,
      d.level !== null ? t('run_level', { level: d.level }) : null,
      d.effort !== null ? t('run_effort', { effort: d.effort }) : null,
      newSession(r) ?? ('started' in session ? t('run_started') : 'resumed' in session ? t('run_continued') : null),
      r.compactions > 0 ? t('run_compacted') : null,
    ]
      .filter((x) => x !== null && x !== '')
      .join(' · ');
  }
  if (r.not_asked !== null) return t('run_not_asked', { need: words(r.not_asked) });
  if (cell?.kind === 'tool') return t('ran_tools', { n: d.tool_runs.length });
  if (cell?.kind === 'owner') return t('ran_owner');
  return cell?.step ? t('ran_step', { step: words(cell.step) }) : t('round_by_engine');
}

/// How long the round took and when it ended.
function tookWords(r: RoundSummary): string {
  if (r.ended_at === null) return t('rp_running_for', { time: duration(Date.now() - r.started_at) });
  return t('took_ended', { took: duration(r.ended_at - r.started_at), at: time(r.ended_at) });
}

/// The overview of a round: who ran it and how long, why it ran, what it led to, and what it cost.
function RoundOverview({ d, rounds, f, cell }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null; cell: CompiledCell | undefined }) {
  const r = d.round;
  const at = rounds.findIndex((x) => x.n === r.n);
  const before = at > 0 ? rounds[at - 1] : undefined;
  const earlier = at > 1 ? rounds[at - 2] : undefined;
  const after = at >= 0 ? rounds[at + 1] : undefined;
  const tool = cell?.kind === 'tool';
  const beforeCell = before?.cell != null ? f?.cell(before.cell) : undefined;
  const why: ReactNode =
    before === undefined ? (
      t('why_started')
    ) : tool ? (
      <>
        <Round f={f} work={d.work} n={before.n} /> {before.cell !== null ? <CellLink f={f} id={before.cell} /> : null} {t('why_asked_tools', { n: d.tool_runs.length })}
      </>
    ) : (
      <>
        {t('why_came')} <Round f={f} work={d.work} n={before.n} /> {before.cell !== null ? <CellLink f={f} id={before.cell} /> : null}
        {before.transition !== null ? (
          <>
            {' '}
            {t('why_by')} <Name>{before.transition}</Name>
          </>
        ) : null}
        {beforeCell?.kind === 'tool' && earlier !== undefined ? (
          <>
            {t('why_with_results')} <Round f={f} work={d.work} n={earlier.n} />
          </>
        ) : null}
        {d.interjections.length > 0 ? t('why_with_words') : null}
      </>
    );
  const row = r.transition !== null ? cell?.rows.find((x) => x.name === r.transition) : undefined;
  const to = row !== undefined && !row.to.startsWith('@') ? row.to : (after?.cell ?? null);
  const led: ReactNode =
    r.outcome === 'running' ? (
      t('rp_running')
    ) : r.transition !== null ? (
      <>
        <Name>{r.transition}</Name> <span className="pp-arrow">→</span> {tool ? `${t('led_back_to')} ` : null}
        {to !== null ? <CellLink f={f} id={to} /> : null}
        {after !== undefined ? (
          <>
            , <Round f={f} work={d.work} n={after.n} />
          </>
        ) : null}
        {tool && after !== undefined ? t('led_given_results') : null}
      </>
    ) : (
      t('led_nothing')
    );
  const flash =
    r.outcome === 'sent_back' ? (
      <Flash variant="warning">{t('rp_sent_back', { why: r.why ?? '' })}</Flash>
    ) : r.outcome === 'failed' || r.outcome === 'interrupted' ? (
      <Flash variant="danger">{t('rp_no_return', { why: r.why ?? '' })}</Flash>
    ) : null;
  const s = r.tokens !== null ? spent(r.tokens, r.model) : null;
  const shares = s?.parts.filter((p) => p.fact !== true) ?? [];
  const facts = s?.parts.filter((p) => p.fact === true) ?? [];
  return (
    <>
      <Overview
        rows={[
          { label: t('ov_ran'), value: ranWords(d, cell), sub: tookWords(r) },
          { label: t('ov_why'), value: why },
          { label: t('ov_led'), value: led },
          {
            label: t('ov_cost'),
            value:
              d.message !== null && r.cost_usd !== null ? (
                <>
                  {joined([
                    cost(r.cost_usd),
                    ...shares.map((p, i) => (
                      <span key={i} title={p.title ?? undefined}>
                        {p.share !== undefined ? <i className={`pp-spent-dot spent-${p.share}`} /> : null}
                        {p.text}
                      </span>
                    )),
                  ])}
                </>
              ) : (
                <span className="pp-facts">{t('cost_none')}</span>
              ),
            sub:
              facts.length > 0 ? (
                <>
                  {joined(
                    facts.map((p, i) => (
                      <span key={i} title={p.title ?? undefined}>
                        {p.text}
                      </span>
                    )),
                  )}
                </>
              ) : undefined,
          },
        ]}
      />
      {flash}
    </>
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

/// A value as it was given to an agent, in the sheet of what is sent: a text as it is, anything else as JSON.
function GivenValue({ value }: { value: JsonValue }) {
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  return (
    <Sheet mono>
      <Long text={text} />
    </Sheet>
  );
}

/// An input the session already held, not sent this round: its name, its address, and once opened its size and what
/// the session held.
function HeldRow({ address, hash, name, d, rounds, f }: {
  address: string;
  hash: string;
  name: string;
  d: RoundDetail;
  rounds: RoundSummary[];
  f: Flow | null;
}) {
  const [value, setValue] = useState<JsonValue | null | undefined>(undefined);
  const load = (open: boolean) => {
    if (open && value === undefined) findHeld(address, hash, d, rounds, f?.g.within ?? 'outcome').then(setValue, () => setValue(null));
  };
  const size = value !== undefined && value !== null ? chars(typeof value === 'string' ? value : JSON.stringify(value, null, 2)) : null;
  return (
    <Row name={name} code={address} facts={size !== null ? t('chars', { n: counted(size) }) : null} onOpen={load}>
      {value === undefined ? (
        <p className="pp-note">{t('loading')}</p>
      ) : value === null ? (
        <p className="pp-note">{t('input_not_found')}</p>
      ) : (
        <GivenValue value={value} />
      )}
    </Row>
  );
}

/// The id of the place in the message where an input is given under its address.
const givenId = (address: string) => `msg-${address.replace(/[^\w-]/g, '_')}`;

/// A message as sent, word for word, in one sheet: every heading and line as it is, a part over 1,000 characters
/// folded with its size and its share of the message, the task saying it is the cell's.
function Message({ text }: { text: string }) {
  const total = chars(text);
  const parts = messageParts(text);
  return (
    <Sheet mono>
      {parts.map((p, i) => {
        if ('heading' in p) {
          const address = p.heading.startsWith('### ') ? p.heading.slice(4).trim() : null;
          return (
            <div key={i} className="msg-h" id={address !== null ? givenId(address) : undefined}>
              {p.heading}
            </div>
          );
        }
        const head = parts[i - 1];
        const task = head !== undefined && 'heading' in head && head.heading === '## Task';
        return (
          <div key={i} className="msg-part">
            <Long text={p.text} more={shareWords(chars(p.text), total, task)} />
          </div>
        );
      })}
    </Sheet>
  );
}

/// The round that made an input given under an address: the latest round before this one at the cell the address
/// names; none for what the engine computed or anything else.
function madeBy(address: string, d: RoundDetail, rounds: RoundSummary[]): RoundSummary | undefined {
  const [head, cell] = address.split('·');
  if (head === undefined || cell === undefined || cell === 'engine' || !/^R\d/.test(head)) return undefined;
  return [...rounds].reverse().find((r) => r.n < d.round.n && r.cell === cell);
}

/// What an agent's round was given: the inputs new this round, those already in its session, and the message as sent.
function AgentGiven({ d, rounds, f, cell }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null; cell: CompiledCell | undefined }) {
  const message = d.message ?? '';
  const parsed = parseMessage(message);
  const sent = d.conditional.filter((c) => c.given);
  const held = d.conditional.filter((c) => !c.given);
  const earlier = rounds.filter((r) => r.n < d.round.n && r.session !== null && r.session === d.round.session);
  const heldFacts =
    earlier.length === 1 ? (
      <>
        {t('held_in')} <Round f={f} work={d.work} n={earlier[0].n} />
        {t('held_not_again')}
      </>
    ) : (
      t('held_earlier')
    );
  return (
    <>
      {sent.length > 0 ? (
        <Part title={t('given_new')}>
          <Rows>
            {sent.map((c) => {
              const by = madeBy(c.address, d, rounds);
              const given = parsed.given.get(c.address);
              return (
                <Row
                  key={c.address}
                  name={inputWords(c.address, cell, f, parsed.workFrom)}
                  code={c.address}
                  facts={joined([
                    given !== undefined ? t('chars', { n: counted(chars(given)) }) : null,
                    by !== undefined ? (
                      <span key="by">
                        {t('given_from')} <Round f={f} work={d.work} n={by.n} />
                      </span>
                    ) : null,
                    <LinkButton
                      key="go"
                      onClick={() => document.getElementById(givenId(c.address))?.scrollIntoView({ block: 'start', behavior: 'smooth' })}
                    >
                      {t('given_in_message')}
                    </LinkButton>,
                  ])}
                />
              );
            })}
          </Rows>
        </Part>
      ) : null}
      {held.length > 0 ? (
        <FoldPart title={t('given_held')} n={held.length} facts={heldFacts}>
          <Rows>
            {held.map((c) => (
              <HeldRow
                key={c.address}
                address={c.address}
                hash={c.hash}
                name={inputWords(c.address, cell, f, parsed.workFrom)}
                d={d}
                rounds={rounds}
                f={f}
              />
            ))}
          </Rows>
        </FoldPart>
      ) : null}
      {d.conditional.length === 0 && d.inputs.length > 0 ? (
        <Part title={t('given_recorded')}>
          <Rows>
            {d.inputs.map((x, i) => (
              <Row key={i} name={brief(x.what)}>
                <GivenValue value={x.value} />
              </Row>
            ))}
          </Rows>
        </Part>
      ) : null}
      <Part title={t('given_as_sent')} facts={t('chars', { n: counted(chars(message)) })}>
        <Message text={message} />
      </Part>
    </>
  );
}

/// What an agent's round handed back: the form its return holds, each field of the form under its title, the fields
/// every return holds, and the return as JSON, folded.
function AgentReturned({ d, cell, f }: { d: RoundDetail; cell: CompiledCell | undefined; f: Flow | null }) {
  const within = f?.g.within ?? 'outcome';
  if (d.output === null) {
    return <p className="pp-note">{d.round.outcome === 'running' ? t('rp_running') : t('returned_nothing')}</p>;
  }
  const options: ShapeOptions = { within, titles: cell?.titles };
  const shown = shape(d.output, cell?.schema, options);
  const fields = shown.kind === 'fields' ? shown.fields : [];
  const form = fields.find((x) => x.key === within);
  const rest = fields.filter((x) => x !== form);
  const schema = isObj(cell?.schema) && isObj(cell.schema.properties) ? cell.schema.properties[within] : undefined;
  const alts = isObj(schema) && Array.isArray(schema.anyOf) ? schema.anyOf : [];
  const value = isObj(d.output) ? d.output[within] : undefined;
  const at = value !== undefined ? alts.findIndex((a) => isObj(a) && isObj(a.properties) && isObj(value) && Object.keys(value).every((k) => k in (a.properties as Obj))) : -1;
  const which = at >= 0 ? cell?.forms?.[at] : undefined;
  const json = JSON.stringify(d.output, null, 2);
  return (
    <>
      {form !== undefined ? (
        <p className="pp-note">
          {inline(
            which === undefined || which.own
              ? t('returned_form_own', { within: `\`${within}\`` })
              : t('returned_form', { form: words(which.field ?? ''), within: `\`${within}\`` }),
          )}
        </p>
      ) : null}
      {form !== undefined && form.value.kind === 'fields'
        ? form.value.fields.map((x) => (
            <div key={x.key} className="pp-part">
              <Named title={x.title ?? titleOf(cell?.titles, x.key, x.key)} code={x.key} as="h3" n={x.value.kind === 'list' ? x.value.items.length : undefined} />
              <Value v={x.value} level={3} />
            </div>
          ))
        : null}
      {rest.length > 0 ? (
        <Part title={form !== undefined ? t('back_every') : t('returned_fields')}>
          {rest.map((x) => (
            <FieldValue key={x.key} f={x} level={4} />
          ))}
        </Part>
      ) : null}
      <FoldPart title={t('returned_json')} facts={t('chars', { n: counted(chars(json)) })}>
        <Said mono>
          <Long text={json} />
        </Said>
      </FoldPart>
    </>
  );
}

/// What a tool was asked, as the agent that asked handed it back: its one text as it is, else each of its values
/// under its name.
function Asked({ args, titles, tool }: { args: JsonValue; titles: Record<string, string> | undefined; tool: string }) {
  if (typeof args === 'string') return <Said mono>{args}</Said>;
  if (isObj(args)) {
    // The tool asked for is the heading already.
    const entries = Object.entries(args).filter(([k, v]) => !(k === 'tool' && v === tool));
    if (entries.length === 1 && typeof entries[0][1] === 'string') return <Said mono>{entries[0][1]}</Said>;
    return (
      <Said mono>
        {entries.map(([k, v]) => (
          <div key={k}>
            <InnerName>{titleOf(titles, `tools[${tool}].${k}`, k)}</InnerName> {typeof v === 'string' ? v : JSON.stringify(v)}
          </div>
        ))}
      </Said>
    );
  }
  return <Said mono>{JSON.stringify(args)}</Said>;
}

/// A short value a tool returned, which goes on its first line by its name.
const short = (v: JsonValue) => typeof v !== 'string' || (chars(v) <= 60 && !v.includes('\n'));

/// What a tool returned, as the round after gives it to the agent that asked, word for word: its short values by
/// their names on the first line, then each long one, folded past 1,000 characters.
function Returned({ result: whole }: { result: JsonValue }) {
  const result = unwrapped(whole);
  if (!isObj(result)) {
    return (
      <Sheet mono>
        <Long text={typeof result === 'string' ? result : JSON.stringify(result, null, 2)} />
      </Sheet>
    );
  }
  const entries = Object.entries(result);
  const line = entries.filter(([, v]) => short(v) && v !== '');
  const empty = entries.filter(([, v]) => v === '');
  const long = entries.filter(([, v]) => !short(v));
  const text = (v: JsonValue) => (typeof v === 'string' ? v : JSON.stringify(v, null, 2));
  return (
    <Sheet mono>
      <div>
        {joined([
          ...line.map(([k, v]) => (
            <span key={k}>
              <InnerName>{k}</InnerName> <Name>{typeof v === 'string' ? v : JSON.stringify(v)}</Name>
            </span>
          )),
          ...empty.map(([k]) => (
            <span key={k}>
              <InnerName>{k}</InnerName> <InnerName>{t('rp_empty')}</InnerName>
            </span>
          )),
          long.length > 0 ? <InnerName key="long">{long[0][0]}</InnerName> : null,
        ])}
      </div>
      {long.map(([k, v], i) => (
        <div key={k}>
          {i > 0 ? <InnerName>{k}</InnerName> : null}
          <Long text={text(v)} />
        </div>
      ))}
    </Sheet>
  );
}

/// What a tool returned, out of the one key the engine records it under, such as `ran`.
function unwrapped(result: JsonValue): JsonValue {
  if (!isObj(result)) return result;
  const entries = Object.entries(result);
  return entries.length === 1 && isObj(entries[0][1]) ? entries[0][1] : result;
}

/// The size of what a tool returned, in its long values; none when it returned nothing more than its verdict.
function returnedSize(whole: JsonValue | null): number {
  if (whole === null) return 0;
  const result = unwrapped(whole);
  if (!isObj(result)) return chars(typeof result === 'string' ? result : JSON.stringify(result));
  return Object.values(result)
    .filter((v) => !short(v))
    .reduce<number>((n, v) => n + chars(typeof v === 'string' ? v : JSON.stringify(v)), 0);
}

/// A file a tool kept, read when asked for.
function KeptFile({ file, path }: { file: string; path: string }) {
  const [text, setText] = useState<string | null>(null);
  const open = (o: boolean) => {
    if (!o || text !== null) return;
    const [dir, ...name] = file.split('/');
    getText(`/api/v1/files/${encodeURIComponent(dir)}/${encodeURIComponent(name.join('/'))}`).then(setText, (e: unknown) => setText(String(e)));
  };
  return (
    <Row name={file} facts={path} onOpen={open}>
      {text === null ? (
        <p className="pp-note">{t('loading')}</p>
      ) : (
        <Sheet mono>
          <Long text={text} />
        </Sheet>
      )}
    </Row>
  );
}

/// Each tool's question and answer together, one tool under another.
function Tools({ runs, asker }: { runs: ToolRun[]; asker: CompiledCell | undefined }) {
  return (
    <>
      {runs.map((run, i) => {
        const size = returnedSize(run.result);
        const took = run.ended_at !== null && run.started_at !== null ? run.ended_at - run.started_at : null;
        return (
          <div key={run.id} className="pp-tool">
            <div className="pp-heading">
              <span className="pp-no">{i + 1}</span>
              <h3 className="pp-sub">
                <Name>{run.tool}</Name>
              </h3>
              <span className="pp-facts">
                {joined([run.attempt > 1 ? t('attempt', { n: run.attempt }) : null, took !== null && took >= 1000 ? duration(took) : null])}
              </span>
            </div>
            <div className="pp-tool-pair">
              <span className="pp-over">{t('tool_asked')}</span>
              <Asked args={run.args} titles={asker?.titles} tool={run.tool} />
            </div>
            <div className="pp-tool-pair">
              <span className="pp-over">
                {run.result === null ? t('tool_running') : size > 0 ? t('tool_returned_n', { n: counted(size) }) : t('tool_returned_nothing')}
              </span>
              {run.result !== null ? <Returned result={run.result} /> : null}
            </div>
            {run.files.length > 0 ? (
              <div className="pp-tool-pair">
                <span className="pp-over">{t('rp_files')}</span>
                <Rows>
                  {run.files.map((x) => (
                    <KeptFile key={x.file} file={x.file} path={x.path} />
                  ))}
                </Rows>
              </div>
            ) : null}
          </div>
        );
      })}
    </>
  );
}

/// A value an engine's step read or came to, in the page's own words: a text as a paragraph, a number or a kind as a
/// code, an object's fields as labelled values, a list as rows.
function PageValue({ v }: { v: Shown }) {
  switch (v.kind) {
    case 'text':
      return <span className="pp-plain-text">{v.text}</span>;
    case 'plain':
    case 'fixed':
      return <Name>{v.text}</Name>;
    case 'empty':
      return <span className="pp-facts">{t('rp_empty')}</span>;
    case 'fields':
      return (
        <dl className="overview">
          {v.fields.map((x) => (
            <div key={x.key} className="overview-row">
              <dt>{x.title ?? x.key}</dt>
              <dd>
                <PageValue v={x.value} />
              </dd>
            </div>
          ))}
        </dl>
      );
    case 'list':
      return (
        <div className="pp-rows">
          {v.items.map((x, i) => (
            <div key={i} className="pp-row-line">
              <span className="pp-no">{i + 1}</span>
              <div>
                <PageValue v={x} />
              </div>
            </div>
          ))}
        </div>
      );
  }
}

/// The schema an input of a step that is not an agent's is read by: that of the place of the return it comes from,
/// in the return format of the cell that gave it (the cell the work came from, for `@caller` and `@from`); none for
/// what the engine, the configuration or the database gives.
function inputSchema(x: RoundDetail['inputs'][number], before: RoundSummary | undefined, f: Flow | null): { schema: Obj | null; agent: boolean; titles?: Record<string, string>; path: string } {
  const src = x.from[0];
  if (!Array.isArray(src)) return { schema: null, agent: false, path: '' };
  const [at, path] = src;
  const id = at.startsWith('@') ? before?.cell : at;
  const c = id != null ? f?.cell(id) : undefined;
  return { schema: placeSchema(c?.schema, path), agent: c?.kind === 'agent', titles: c?.titles, path };
}

/// The name of an input of a step, as its cell names it, by what it is.
function stepInputName(x: RoundDetail['inputs'][number], cell: CompiledCell | undefined): string {
  const own = cell?.inputs.find((y) => y.what === x.what);
  return own !== undefined ? inputName(own) : brief(x.what);
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
  const asker = before?.cell != null ? f?.cell(before.cell) : undefined;
  const [open, setOpen] = useState<Set<string>>(() => new Set(roundOpen));
  useEffect(() => {
    roundOpen = open;
  }, [open]);
  const section = (key: string) => ({
    open: open.has(key),
    onOpen: (o: boolean) =>
      setOpen((was) => {
        if (was.has(key) === o) return was;
        const n = new Set(was);
        if (o) n.add(key);
        else n.delete(key);
        return n;
      }),
  });
  const sent = d.conditional.filter((c) => c.given).length;
  const held = d.conditional.length - sent;
  const json = d.output !== null ? JSON.stringify(d.output, null, 2) : '';
  return (
    <div className="pp-page">
      <RoundOverview d={d} rounds={rounds} f={f} cell={cell} />
      {agent ? (
        <>
          <PanelSection title={t('rp_given')} facts={joined([t('given_new_n', { n: sent }), held > 0 ? t('given_held_n', { n: held }) : null])} {...section('given')}>
            <AgentGiven d={d} rounds={rounds} f={f} cell={cell} />
          </PanelSection>
          <PanelSection
            title={t('rp_returned')}
            facts={
              r.transition !== null
                ? t('returned_took', { row: r.transition })
                : r.outcome === 'sent_back'
                  ? t('outcome_sent_back')
                  : undefined
            }
            {...section('returned')}
          >
            <AgentReturned d={d} cell={cell} f={f} />
          </PanelSection>
        </>
      ) : tool ? (
        <PanelSection
          title={t('rp_tools')}
          n={d.tool_runs.length}
          facts={
            before !== undefined ? (
              <>
                {t('tools_as_asked')} <Round f={f} work={d.work} n={before.n} />
                {t('tools_asked_them')}
                {after !== undefined ? (
                  <>
                    {t('tools_given_back')} <Round f={f} work={d.work} n={after.n} />
                    {t('tools_word_for_word')}
                  </>
                ) : null}
              </>
            ) : undefined
          }
          {...section('tools')}
        >
          {/* What an agent handed back that the tools run after, such as the architect's reply to a failure. */}
          {d.inputs
            .filter((x) => !empty(x.value) && Array.isArray(x.from[0]) && !String(x.from[0][1]).startsWith('tools'))
            .map((x, i) => {
              const s = inputSchema(x, before, f);
              return (
                <div key={i} className="pp-part">
                  <Named title={stepInputName(x, cell)} />
                  <Value v={shape(x.value, s.schema, { titles: s.titles })} level={3} />
                </div>
              );
            })}
          <Tools runs={d.tool_runs} asker={asker} />
          {d.output !== null ? (
            <FoldPart title={t('tools_json')} facts={t('chars', { n: counted(chars(json)) })}>
              <Sheet mono>
                <Long text={json} />
              </Sheet>
            </FoldPart>
          ) : null}
        </PanelSection>
      ) : (
        <>
          {d.inputs.length > 0 ? (
            <PanelSection title={t('rp_step_read')} n={d.inputs.length} {...section('read')}>
              {d.inputs.map((x, i) => {
                const s = inputSchema(x, before, f);
                const v = shape(x.value, s.schema, { titles: s.titles });
                return (
                  <div key={i} className="pp-part">
                    <Named title={stepInputName(x, cell)} />
                    {s.agent ? <Value v={v} level={3} /> : <PageValue v={v} />}
                  </div>
                );
              })}
            </PanelSection>
          ) : null}
          <PanelSection title={t('rp_step_came')} {...section('came')}>
            {d.output !== null ? <PageValue v={shape(d.output, cell?.schema, { titles: cell?.titles })} /> : <p className="pp-note">{t('returned_nothing')}</p>}
            {d.tool_runs.length > 0 ? (
              <Part title={t('rp_step_tools')}>
                <Tools runs={d.tool_runs} asker={undefined} />
              </Part>
            ) : null}
            {d.asks.map((a) => (
              <AskView key={a.id} a={a} />
            ))}
            {d.output !== null ? (
              <FoldPart title={t('returned_json_step')} facts={t('chars', { n: counted(chars(json)) })}>
                <pre className="code-block">{json}</pre>
              </FoldPart>
            ) : null}
          </PanelSection>
        </>
      )}
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
