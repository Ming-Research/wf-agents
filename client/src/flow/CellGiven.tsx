// What a cell is given, as its panel's first section shows it. For an agent's cell: the task, sent every round, in the
// sheet of what is sent to an agent, folded to its first lines with its size; under it, folded, the whole message with
// what each round fills in; the inputs in the order the message numbers them, each opening to what the agent is told
// it is and to what it holds as far as the page can know outside a round; and the rules and formats, each opening to
// its text. For an engine's step, a tool or the owner: what the step does, and the inputs it reads, each opening to
// what it holds; none of it goes into any agent's context.

import type { ReactNode } from 'react';

import type { CellInput, CompiledCell, CompiledInput, InputsNow, Src } from '../api';
import { useResource } from '../live';
import { Markdown } from '../markdown';
import { t, tn } from '../text';
import { charCount, chars, Code, Filled, FoldPart, Long, Name, Over, Part, Row, Rows, Sheet } from '../voices';
import { useFlow } from './context';
import { CellCode, inline, sheetText } from './words';

/// Whether an input is the project's own context, which the message gives before anything else: the project's guide
/// and a setting of its configuration, such as its design goal.
export function projectOwn(x: CompiledInput): boolean {
  return x.from.some((s) => typeof s === 'object' && !Array.isArray(s) && ('config' in s || ('engine' in s && s.engine === 'project_guide')));
}

/// The cell's inputs, each with the number the message's list to work from gives it, or none for an input the agent
/// reads itself; the engine numbers them so in `inputs:<cell>`.
export function numbered(c: CompiledCell): Array<{ x: CompiledInput; i: number; no: number | null }> {
  let no = 0;
  return c.inputs.map((x, i) => {
    if (x.from.every((s) => s === 'self')) return { x, i, no: null };
    no += 1;
    return { x, i, no };
  });
}

/// An input's name, as the flow gives it; in a flow version stored before every input had one, its description's
/// first clause.
export function inputName(x: CompiledInput): string {
  if (x.name) return x.name;
  const m = /^(.{8,}?)(?:[,.:;](?=\s|$)| \()/.exec(x.what);
  return m !== null ? m[1].trim() : x.what;
}

/// The words for where one source of an input comes from: a cell's return, by the cells it can be; the project's
/// setting; the database; the engine; the agent itself; what rows recorded.
function sourceWords(src: Src, cells: string[]): ReactNode {
  if (Array.isArray(src)) {
    const [at] = src;
    if (!at.startsWith('@')) {
      return (
        <>
          {t('src_cell')} <CellCode id={at} />
        </>
      );
    }
    const who = at === '@from' ? t('src_from') : at === '@caller' ? t('src_caller') : t('src_stopped');
    return (
      <>
        {t('src_place', { who })}
        {cells.map((c, i) => (
          <span key={c}>
            {i === 0 ? ', ' : ' or '}
            <CellCode id={c} />
          </span>
        ))}
      </>
    );
  }
  if (src === 'self') return t('src_self');
  if ('config' in src) return inline(t('src_config', { name: `\`${src.config}\`` }));
  if ('db' in src) {
    const by = src.by === 'work' ? t('src_by_work') : src.by === 'item' ? t('src_by_item') : src.by === 'project' ? t('src_by_project') : null;
    return by !== null ? t('src_db_by', { by }) : inline(t('src_db_named', { name: `\`${src.db}\`` }));
  }
  if ('engine' in src) return t('src_computed');
  return inline(t('src_recorded', { name: `\`${src.recorded}\`` }));
}

/// Where an input comes from, each kind of source once, and whether it may be missing.
export function InputFacts({ x, sources }: { x: CompiledInput; sources: CellInput[] }) {
  const words: ReactNode[] = [];
  const seen = new Set<string>();
  x.from.forEach((src, j) => {
    const key = Array.isArray(src) ? `place:${src[0]}` : typeof src === 'string' ? src : Object.keys(src)[0];
    if (seen.has(key)) return;
    seen.add(key);
    const m = sources.find((y) => y.of.kind === 'input' && y.of.source === j);
    words.push(<span key={key}>{sourceWords(src, m?.cells ?? [])}</span>);
  });
  return (
    <>
      {words.map((w, i) => (
        <span key={i}>
          {i > 0 ? ', ' : null}
          {w}
        </span>
      ))}
      {x.maybe ? ` · ${t('input_if_any')}` : null}
    </>
  );
}

/// What one source of an input holds now, as far as the page can know outside a round: a setting, as each project's
/// configuration has it, in the sheet of what is sent; the project guide, as each project's checkout has it; what the
/// engine computes and a table or view, as the flow and the definition say, in the page's words; nothing for a place
/// of a return, which the cell it comes from shows, or what rows recorded.
function SourceHolds({ src, now }: { src: Src; now: InputsNow | undefined }) {
  const f = useFlow();
  if (Array.isArray(src) || src === 'self' || 'recorded' in src) return null;
  if ('engine' in src && src.engine !== 'project_guide') {
    return <p className="pp-note">{inline(t('holds_engine', { what: f.compiled.engine.inputs[src.engine] ?? src.engine }))}</p>;
  }
  if (now === undefined) return <p className="pp-note">{t('loading')}</p>;
  if ('db' in src) {
    const table = now.tables.find((x) => x.name === src.db);
    return table !== undefined ? (
      <p className="pp-note">
        {t('holds_table')} <Name>{src.db}</Name>: {inline(table.text)}
      </p>
    ) : null;
  }
  if ('config' in src) {
    return (
      <>
        {now.projects.map((p) => {
          const value = p.settings.find((s) => s.name === src.config);
          return (
            <div key={p.project} className="pp-part">
              <Over>{t('holds_now_in', { project: p.project })}</Over>
              {value !== undefined ? (
                <Sheet>
                  <Long text={value.text}>{(x) => <Markdown text={x} />}</Long>
                </Sheet>
              ) : (
                <p className="pp-note">{t('holds_unset')}</p>
              )}
            </div>
          );
        })}
      </>
    );
  }
  return (
    <>
      {now.projects.map((p) => (
        <div key={p.project} className="pp-part">
          <Over>{tn('holds_guide_in', p.design_tree.length, { project: p.project })}</Over>
          <Sheet>
            <Long text={p.agents_md}>{(x) => <Markdown text={x} />}</Long>
          </Sheet>
          <Sheet mono>
            <Long text={p.design_tree.map((n) => `${n.name} ${n.text}`).join('\n')} />
          </Sheet>
        </div>
      ))}
      <p className="pp-note">{t('holds_guide_cited')}</p>
    </>
  );
}

/// One numbered input of an agent's cell: its number, name, addresses and where it comes from; opening to what the
/// agent is told it is, in the list of the cell's inputs (and the forms every return shares, for an input every agent's
/// cell has), and to what it holds.
function AgentInput(props: {
  c: CompiledCell;
  no: number;
  x: CompiledInput;
  i: number;
  sources: CellInput[];
  now: InputsNow | undefined;
  open: boolean;
  onOpen: (open: boolean) => void;
}) {
  const { c, no, x, i, sources, now, open, onOpen } = props;
  const addresses = sources.map((m) => m.address ?? '').filter((a) => a !== '');
  const told = x.common ? t('told_in_both', { cell: c.id }) : t('told_in', { cell: c.id });
  return (
    <Row
      id={`cell-input-${i}`}
      no={no}
      name={inputName(x)}
      code={addresses.join(' ')}
      facts={<InputFacts x={x} sources={sources} />}
      open={open}
      onOpen={onOpen}
    >
      <Over>{inline(told)}</Over>
      <Sheet>{sheetText(x.what)}</Sheet>
      {x.from.map((src, j) => (
        <SourceHolds key={j} src={src} now={now} />
      ))}
    </Row>
  );
}

/// The message as it is sent every time the cell is entered, in the order sent, what each round fills in drawn as a
/// placeholder.
function WholeMessage({ c, inputs }: { c: CompiledCell; inputs: CellInput[] }) {
  const own = numbered(c);
  const sources = (i: number) => inputs.filter((m) => m.of.kind === 'input' && m.of.input === i);
  const others = inputs.filter((m) => m.of.kind !== 'input');
  const rules = others.filter((m) => m.of.kind === 'project_rules' || m.of.kind === 'rule').map((m) => m.address ?? '');
  const worktree = others.find((m) => m.of.kind === 'workspace')?.address ?? null;
  // The table review is given the columns of the review table the rule of that name lists, each with its number.
  const table = (c.rules ?? []).some((r) => r.name === 'review_table');
  const work = own.flatMap(({ x, i, no }) => {
    if (no === null) return [t('sent_read_yourself', { what: inputName(x) })];
    const addresses = sources(i)
      .map((m) => m.address ?? '')
      .filter((a) => a !== '');
    return [`${no}. ${addresses.join(', ')}`];
  });
  if (rules.length > 0) work.push(t('sent_rules', { rules: rules.join(', ') }));
  if (worktree !== null) work.push(t('sent_worktree', { address: worktree }));
  return (
    <Sheet mono>
      <div className="msg-h">
        <Filled text={t('sent_title', { cell: c.id, title: c.title })} />
      </div>
      <div className="msg-part">
        <Filled text={`‹${t('sent_why')}›`} />
      </div>
      <div className="msg-h">{t('sent_given')}</div>
      <div className="msg-part">
        <Filled text={`‹${t('sent_given_what')}›`} />
      </div>
      <div className="msg-h">{t('sent_task')}</div>
      <div className="msg-part">
        <Filled text={`‹${t('sent_task_above')}›`} />
      </div>
      <div className="msg-h">{t('sent_work_from')}</div>
      <div className="msg-part">
        <Filled text={work.join('\n')} />
      </div>
      {table ? (
        <>
          <div className="msg-h">{t('sent_table')}</div>
          <div className="msg-part">
            <Filled text={`‹${t('sent_table_what')}›`} />
          </div>
        </>
      ) : null}
      <div className="msg-part">{`\n${t('sent_hand_in', { cell: c.id })}`}</div>
    </Sheet>
  );
}

/// The task, sent every round, word for word; a task that differs by a value the engine knows, each case after it.
function Task({ c }: { c: CompiledCell }) {
  const prompt = c.prompt ?? [];
  const text = prompt.join('\n\n');
  const enums = c.prompt_by ? (c.inenums?.[c.prompt_by.by] ?? []) : [];
  return (
    <Part title={t('given_task')} facts={t('given_task_facts', { chars: charCount(chars(text)) })}>
      <Sheet>
        <Long text={text}>
          {() => (
            <>
              {prompt.map((p, i) => (
                <p key={i}>{sheetText(p)}</p>
              ))}
            </>
          )}
        </Long>
      </Sheet>
      {c.prompt_by
        ? Object.entries(c.prompt_by.cases).map(([k, lines]) => (
            <div key={k} className="pp-part">
              <Over>
                {inline(t('given_task_case', { by: `\`${c.prompt_by?.by ?? ''}\``, value: `\`${k}\`` }))}
                {enums.find(([v]) => v === k)?.[1] ? ` (${enums.find(([v]) => v === k)?.[1] ?? ''})` : null}
              </Over>
              <Sheet>
                {lines.map((l, i) => (
                  <p key={i}>{sheetText(l)}</p>
                ))}
              </Sheet>
            </div>
          ))
        : null}
    </Part>
  );
}

/// A rule or a format the cell is given once in a session: its name, its address and its size, opening to its text.
function Given({ m, c, now }: { m: CellInput; c: CompiledCell; now: InputsNow | undefined }) {
  const f = useFlow();
  const address = m.address ?? '';
  if (m.of.kind === 'rule') {
    const name = m.of.name;
    const rule = f.g.rules.find((r) => r.name === name);
    const text = (rule?.text ?? []).join('\n');
    return (
      <Row name={t('given_rule', { name })} code={address} facts={rule ? charCount(chars(text)) : null}>
        {rule ? (
          <Sheet>
            <Long text={text}>
              {() => (
                <>
                  {rule.text.map((line, i) => (
                    <p key={i}>{sheetText(line)}</p>
                  ))}
                </>
              )}
            </Long>
          </Sheet>
        ) : null}
      </Row>
    );
  }
  if (m.of.kind === 'project_rules') {
    return (
      <Row name={t('given_project_rules')} code={address} facts={t('given_project_rules_facts')}>
        {now === undefined ? (
          <p className="pp-note">{t('loading')}</p>
        ) : (
          now.projects.map((p) => (
            <div key={p.project} className="pp-part">
              <Over>{t('holds_now_in', { project: p.project })}</Over>
              {p.rules.length === 0 ? <p className="pp-note">{t('none')}</p> : null}
              {p.rules.map((r) => (
                <div key={r.name} className="pp-part">
                  <Over>
                    <Code>{`rule:${r.name}`}</Code>
                  </Over>
                  <Sheet>
                    <Long text={r.text}>{(x) => <Markdown text={x} />}</Long>
                  </Sheet>
                </div>
              ))}
            </div>
          ))
        )}
      </Row>
    );
  }
  if (m.of.kind === 'workspace') {
    return (
      <Row name={t('given_workspace')} code={address} facts={t('given_workspace_facts')}>
        <p className="pp-note">{t('holds_workspace')}</p>
      </Row>
    );
  }
  const text = m.of.kind === 'common_format' ? f.view.common_format : (m.holds ?? '');
  const name =
    m.of.kind === 'common_format'
      ? t('given_common_format')
      : m.of.kind === 'cell_format'
        ? t('given_cell_format')
        : t('given_cell_inputs', { cell: c.id });
  return (
    <Row name={name} code={address} facts={text !== '' ? charCount(chars(text)) : null}>
      {text !== '' ? (
        <Sheet mono>
          <Long text={text} />
        </Sheet>
      ) : null}
    </Row>
  );
}

/// The order the rules and formats are listed in: the rules, the project's, the formats, the worktree.
const ORDER: Record<CellInput['of']['kind'], number> = {
  rule: 0,
  project_rules: 1,
  common_format: 2,
  cell_format: 3,
  cell_inputs: 4,
  workspace: 5,
  input: 6,
};

/// What an agent's cell is given.
export function AgentGiven({ c, opened, onOpened }: {
  c: CompiledCell;
  /// The inputs open, by their place among the cell's inputs, and how one opens or folds.
  opened: Set<number>;
  onOpened: (i: number, open: boolean) => void;
}) {
  const f = useFlow();
  const inputs = f.cellView(c.id)?.inputs ?? [];
  // What the inputs from outside the flow hold now: the projects' settings, guides and rules, and the database.
  const now = useResource<InputsNow>('inputs-now', '/api/v1/inputs-now').data;
  const sources = (i: number) => inputs.filter((m) => m.of.kind === 'input' && m.of.input === i);
  const others = inputs.filter((m) => m.of.kind !== 'input').sort((a, b) => ORDER[a.of.kind] - ORDER[b.of.kind]);
  return (
    <>
      <Task c={c} />
      <FoldPart title={t('given_message')} facts={t('given_message_facts')}>
        <WholeMessage c={c} inputs={inputs} />
      </FoldPart>
      <Part title={t('given_inputs')} facts={t('given_inputs_facts')}>
        <Rows>
          {numbered(c).map(({ x, i, no }) =>
            no === null ? (
              <Row key={i} name={inputName(x)} facts={t('src_self')}>
                <Over>{t('told_read_yourself')}</Over>
                <Sheet>{sheetText(x.what)}</Sheet>
              </Row>
            ) : (
              <AgentInput
                key={i}
                c={c}
                no={no}
                x={x}
                i={i}
                sources={sources(i)}
                now={now}
                open={opened.has(i)}
                onOpen={(o) => onOpened(i, o)}
              />
            ),
          )}
        </Rows>
      </Part>
      {others.length > 0 ? (
        <Part title={t('given_rules')} facts={t('given_rules_facts')}>
          <Rows>
            {others.map((m, i) => (
              <Given key={i} m={m} c={c} now={now} />
            ))}
          </Rows>
        </Part>
      ) : null}
    </>
  );
}

/// What a step that is no agent's round does and reads, in the page's words: none of it goes into any agent's context.
export function StepGiven({ c, opened, onOpened }: {
  c: CompiledCell;
  opened: Set<number>;
  onOpened: (i: number, open: boolean) => void;
}) {
  const f = useFlow();
  const inputs = f.cellView(c.id)?.inputs ?? [];
  const now = useResource<InputsNow>('inputs-now', '/api/v1/inputs-now').data;
  const sources = (i: number) => inputs.filter((m) => m.of.kind === 'input' && m.of.input === i);
  return (
    <>
      {c.step ? (
        <Part title={t('given_step')} code={c.step}>
          <p className="pp-note">{inline(f.compiled.engine.steps[c.step] ?? '')}</p>
          {c.needs ? (
            <p className="pp-note">
              {inline(t('step_needs', { need: `\`${c.needs}\`` }))} {inline(f.compiled.engine.needs[c.needs] ?? '')}
            </p>
          ) : null}
        </Part>
      ) : null}
      <Part title={t('given_reads')} facts={t('given_reads_facts')}>
        {c.inputs.length === 0 ? <p className="pp-note">{t('none')}</p> : null}
        <Rows>
          {c.inputs.map((x, i) => (
            <Row
              key={i}
              id={`cell-input-${i}`}
              name={inputName(x)}
              facts={<InputFacts x={x} sources={sources(i)} />}
              open={opened.has(i)}
              onOpen={(o) => onOpened(i, o)}
            >
              <p className="pp-note">{inline(x.what)}</p>
              {x.from.map((src, j) => (
                <SourceHolds key={j} src={src} now={now} />
              ))}
            </Row>
          ))}
        </Rows>
      </Part>
    </>
  );
}
