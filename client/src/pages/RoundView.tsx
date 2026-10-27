// A round, as the owner reads it. First, in a sentence or two, what happened: who did the work (an agent, the engine
// itself, tools, the owner), how it ended, and where the work came from and went. For an agent's round, then, its
// inputs (each input of its cell with the addresses it had this round), what it was actually given once repeats were
// taken out (each given input with its content; those its session already held, only named), and what it handed back.
// For a step that is not an agent's, what it read and what it came to; for tools, what each did. The whole message as
// sent is folded at the end.

import { useContext, useState } from 'react';

import type { RoundDetail, RoundSummary, ToolRun } from '../api';
import { Json, Loaded, Section } from '../components';
import { FlowContext, type Flow } from '../flow/context';
import { cost, duration, time } from '../format';
import { getText } from '../http';
import { useResource } from '../live';
import { srcText, t } from '../text';

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

function Tool({ run }: { run: ToolRun }) {
  return (
    <div className="boxed tool-run">
      <div>
        <strong>{run.tool}</strong>
        {run.attempt > 1 ? <span className="muted"> {t('attempt', { n: run.attempt })}</span> : null}
        {run.ended_at !== null && run.started_at !== null ? (
          <span className="muted"> · {duration(run.ended_at - run.started_at)}</span>
        ) : null}
      </div>
      <Json value={run.result} />
      <details>
        <summary className="muted">{t('tool_asked')}</summary>
        <Json value={run.args} />
      </details>
      {run.files.map((f) => (
        <KeptFile key={f.file} file={f.file} path={f.path} />
      ))}
    </div>
  );
}

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

/// What an address names, in words.
function addressWords(a: string): string {
  const [kind, rest] = a.includes(':') && !a.startsWith('R') ? [a.slice(0, a.indexOf(':')), a.slice(a.indexOf(':') + 1)] : ['', a];
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
      return t('addr_setting', { name: rest });
    case 'ij':
      return t('addr_interjection');
    case 'design_card':
      return t('addr_design_card', { id: rest });
    case 'ask':
      return t('addr_ask', { id: rest });
  }
  const parts = a.split('·');
  if (parts.length >= 3 && parts[1] === 'engine') return t('addr_engine', { name: parts.slice(2).join('·'), round: parts[0] });
  if (parts.length >= 3) return t('addr_value', { path: parts.slice(2).join('·'), cell: parts[1], round: parts[0] });
  return a;
}

/// A cell by its id and title.
function cellName(f: Flow | null, id: string | null): string {
  if (id === null) return '';
  const c = f?.cell(id);
  return c ? `${id} (${c.title})` : id;
}

/// What happened in the round, in a sentence or two.
function Happened({ d, rounds, f }: { d: RoundDetail; rounds: RoundSummary[]; f: Flow | null }) {
  const r = d.round;
  const cell = r.cell !== null ? f?.cell(r.cell) : undefined;
  const at = rounds.findIndex((x) => x.id === r.id);
  const before = at > 0 ? rounds[at - 1] : undefined;
  const after = at >= 0 && at + 1 < rounds.length ? rounds[at + 1] : undefined;
  const ran = d.message !== null;
  const progress = d.output !== null && typeof d.output === 'object' && !Array.isArray(d.output) ? d.output.progress : undefined;
  const who = ran
    ? t('happened_agent', {
        line: r.line ?? 'engineer',
        how: [
          r.ended_at !== null ? duration(r.ended_at - r.started_at) : null,
          r.model,
          r.cost_usd !== null ? cost(r.cost_usd) : null,
        ]
          .filter((x) => x !== null && x !== '')
          .join(', '),
      })
    : cell?.kind === 'agent'
      ? typeof progress === 'string'
        ? t('happened_no_agent')
        : t('happened_engine_answered')
      : cell?.kind === 'tool'
        ? t('happened_tools')
        : cell?.kind === 'owner'
          ? t('happened_owner')
          : t('happened_engine', { step: cell?.step ?? '' });
  const outcome =
    r.outcome === 'took'
      ? after !== undefined
        ? t('happened_went', { cell: cellName(f, after.cell), row: r.transition ?? '' })
        : t('happened_took', { row: r.transition ?? '' })
      : r.outcome === 'sent_back'
        ? t('happened_sent_back', { why: r.why ?? '' })
        : r.outcome === 'failed'
          ? t('happened_failed', { why: r.why ?? '' })
          : r.outcome === 'interrupted'
            ? t('happened_cut')
            : r.outcome === 'running'
              ? t('happened_running', { time: duration(Date.now() - r.started_at) })
              : '';
  const from =
    before !== undefined && before.cell !== r.cell
      ? t('happened_from', { cell: cellName(f, before.cell), row: before.transition ?? '' })
      : null;
  return (
    <div className="round-what">
      <p>
        {from !== null ? `${from} ` : ''}
        {who}
        {typeof progress === 'string' && !ran ? ` ${progress}` : ''}
      </p>
      <p>
        {outcome}
        {r.ended_at !== null ? <span className="muted"> {t('happened_at', { time: time(r.ended_at) })}</span> : null}
        {r.compactions > 0 ? <span className="muted"> {t('happened_compacted')}</span> : null}
      </p>
    </div>
  );
}

/// An agent's round: its inputs, what it was given once repeats were taken out, and the task.
function AgentInputs({ d, f }: { d: RoundDetail; f: Flow | null }) {
  const m = parseMessage(d.message ?? '');
  const held = new Map(d.conditional.map((c) => [c.address, c.given]));
  const cell = d.round.cell !== null ? f?.cell(d.round.cell) : undefined;
  const numbered = (cell?.inputs ?? []).filter((x) => !(x.from.length === 1 && x.from[0] === 'self'));
  // The list of inputs to work from gives each input's number with its addresses this round.
  const byNumber = new Map<number, string[]>();
  const others: string[] = [];
  for (const line of m.workFrom) {
    const hit = /^(\d+)\. (.*)$/.exec(line);
    if (hit) byNumber.set(Number(hit[1]), hit[2].split(', '));
    else others.push(line);
  }
  const chip = (a: string) => (
    <span key={a} className={held.get(a) === false ? 'addr addr-held' : 'addr'} title={addressWords(a)}>
      <code>{a}</code>
    </span>
  );
  const given = d.conditional.filter((c) => c.given);
  const notGiven = d.conditional.filter((c) => !c.given);
  return (
    <>
      <Section title={t('round_inputs')}>
        <ol className="round-inputs">
          {numbered.map((x, i) => (
            <li key={i}>
              <span>{x.what}</span>
              <div>{byNumber.has(i + 1) ? byNumber.get(i + 1)?.map(chip) : <span className="muted">{t('round_input_empty')}</span>}</div>
            </li>
          ))}
        </ol>
        {others.map((o) => (
          <div key={o} className="muted">
            {o}
          </div>
        ))}
      </Section>
      <Section title={t('round_given', { given: given.length, held: notGiven.length })}>
        {given.map((c) => (
          <details key={c.address} className="given">
            <summary>
              <code>{c.address}</code> <span className="muted">{addressWords(c.address)}</span>
            </summary>
            <pre className="j-text">{m.given.get(c.address) ?? ''}</pre>
          </details>
        ))}
        {notGiven.length > 0 ? (
          <p className="muted">
            {t('round_held')} {notGiven.map((c) => c.address).join(', ')}
          </p>
        ) : null}
      </Section>
      {m.task !== null ? (
        <Section title={t('round_task')} open={false}>
          <pre className="j-text">{m.task}</pre>
        </Section>
      ) : null}
    </>
  );
}

/// What a step that was not an agent's came to, without the progress the sentence at the top already says; folded then,
/// since the sentence says it in words.
function cameTo(output: RoundDetail['output']): RoundDetail['output'] {
  if (output === null || typeof output !== 'object' || Array.isArray(output) || typeof output.progress !== 'string') {
    return output;
  }
  const rest = Object.fromEntries(Object.entries(output).filter(([k]) => k !== 'progress'));
  return Object.keys(rest).length > 0 ? rest : null;
}

function Detail({ d, rounds }: { d: RoundDetail; rounds: RoundSummary[] }) {
  const f = useContext(FlowContext);
  const ran = d.message !== null;
  const output = ran ? d.output : cameTo(d.output);
  return (
    <div className="round">
      <Happened d={d} rounds={rounds} f={f} />
      {ran ? <AgentInputs d={d} f={f} /> : null}
      {!ran && d.inputs.length > 0 ? (
        <Section title={t('round_read')} open={false}>
          {d.inputs.map((x, i) => (
            <details key={i} className="input">
              <summary>
                <span className="input-what">{x.what}</span>
                {x.from.length > 0 ? <span className="muted"> · {x.from.map(srcText).join('; ')}</span> : null}
              </summary>
              <Json value={x.value} />
            </details>
          ))}
        </Section>
      ) : null}
      {d.tool_runs.length > 0 ? (
        <Section title={t('round_tools', { n: d.tool_runs.length })}>
          {d.tool_runs.map((run) => (
            <Tool key={run.id} run={run} />
          ))}
        </Section>
      ) : null}
      {output !== null ? (
        <Section title={ran ? t('round_returned') : t('round_came_to')} open={ran || output === d.output}>
          <Json value={output} open={4} />
        </Section>
      ) : null}
      {d.design_cards.map((c) => (
        <Section key={`d${c.id}`} title={t('round_design_card', { id: c.id, status: c.status })} open={false}>
          <Json value={c.body} />
        </Section>
      ))}
      {d.asks.map((a) => (
        <Section key={`a${a.id}`} title={t('round_ask', { id: a.id })} open={a.answer === null}>
          <Json value={a.body} />
          {a.answer !== null ? <Json value={a.answer} /> : <p className="muted">{t('ask_waits')}</p>}
        </Section>
      ))}
      {ran ? (
        <Section title={t('message')} open={false}>
          <pre className="j-text prompt">{d.message}</pre>
        </Section>
      ) : null}
    </div>
  );
}

export function RoundView({ id, rounds = [] }: { id: number; rounds?: RoundSummary[] }) {
  const round = useResource<RoundDetail>(`round:${id}`, `/api/v1/rounds/${id}`);
  return <Loaded r={round}>{(d) => <Detail d={d} rounds={rounds} />}</Loaded>;
}
