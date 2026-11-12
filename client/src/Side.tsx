// The panel docked at the chart's right: everything about the cell or block the owner chose, or, when nothing is chosen
// and there is a definition to compare against, the list of what changed, each change as the owner approves it.

import type { ReactNode } from 'react';

import type { Block, Cell, Change, Fault, Flow } from './api';
import type { Chosen } from './Chart';
import { markedChange } from './diff';
import { type BlockRow, type CellColumn, type Matrix, markOf, where } from './matrix';
import { ListRow, Panel, PanelPart, STATUS_WORDS, type Status, StatusLine } from './parts';

/// Whether the panel is folded away, and the way to fold it or bring it back.
export type Fold = { folded: boolean; onFold: () => void; edge?: ReactNode };

/// The status line of what a change did to a thing, when it did anything.
function ChangeStatus({ status, changes }: { status: Status; changes: number }) {
  if (status === 'same') return null;
  return (
    <StatusLine tone={status} word={STATUS_WORDS[status].replace(/^./, (c) => c.toUpperCase())}>
      {status === 'changed' ? `${changes} change${changes === 1 ? '' : 's'}, listed below` : undefined}
    </StatusLine>
  );
}
import {
  BROUGHT_UP_TO_DATE,
  changeWords,
  conditionWords,
  faultWords,
  inputKindWords,
  LIMIT_WORDS,
  placeOf,
  runBy,
  testWords,
  thereWhen,
  type Place,
  writersBesideCells,
} from './words';

/// A name the owner can choose to see it on the chart and in the panel.
function Choose({ chosen, onChoose, children }: { chosen: Chosen; onChoose: (c: Chosen) => void; children: ReactNode }) {
  return (
    <button type="button" className="link" onClick={() => onChoose(chosen)}>
      {children}
    </button>
  );
}

/// What was taken out or put in of a text, each line break in it after a "↵", so that a change of a line break alone
/// shows.
const marked = (text: string) => text.replace(/\n/g, '↵\n');

/// One change: what changed, what it was, and what it is. A task is what its agent is told, so a change of one keeps
/// its line breaks: it stands as one text with the words taken out and put in marked, or, where most of its words
/// changed or it had none or has none, what it was and what it is each as a block of their own, one under the other.
export function ChangeLine({ change }: { change: Change }) {
  const { what, before, after } = changeWords(change);
  const parts = change.kind === 'task' && change.before && change.after ? markedChange(change.before, change.after) : null;
  if (parts) {
    return (
      <span className="change">
        <span className="change-what">{what}</span>
        <span className="change-text">
          {parts.map((part, i) =>
            part.kind === 'same' ? (
              part.text
            ) : part.kind === 'del' ? (
              <del key={i}>{marked(part.text)}</del>
            ) : (
              <ins key={i}>{marked(part.text)}</ins>
            ),
          )}
        </span>
      </span>
    );
  }
  if (change.kind === 'task') {
    return (
      <span className="change">
        <span className="change-what">{what}</span>
        {before && <del className="change-text">{before}</del>}
        {after && <ins className="change-text">{after}</ins>}
      </span>
    );
  }
  return (
    <span className="change">
      <span className="change-what">{what}</span>
      {before && <del>{before}</del>}
      {before && after && <span aria-hidden="true"> → </span>}
      {after && <ins>{after}</ins>}
    </span>
  );
}

/// The list of what changed, in the order the interface gives, each run of changes at one place under its place.
export function ChangesList(props: {
  changes: Change[];
  after: Flow;
  before: Flow;
  onChoose: (c: Chosen) => void;
  fold: Fold;
}) {
  const { changes, after, before, onChoose, fold } = props;
  const groups: { place: Place; changes: Change[] }[] = [];
  for (const change of changes) {
    const place = placeOf(change);
    const last = groups[groups.length - 1];
    if (last && JSON.stringify(last.place) === JSON.stringify(place)) last.changes.push(change);
    else groups.push({ place, changes: [change] });
  }
  const title = (id: string) => (after.cells.find((c) => c.id === id) ?? before.cells.find((c) => c.id === id))?.title;
  const head = (place: Place) => {
    switch (place.on) {
      case 'flow':
        return 'The flow';
      case 'test':
        return (
          <>
            Test <span className="mono">{place.test}</span>
          </>
        );
      case 'cell':
        return (
          <Choose chosen={{ cell: place.cell }} onChoose={onChoose}>
            <span className="mono">{place.cell}</span> {title(place.cell)}
          </Choose>
        );
      case 'block':
        return (
          <Choose chosen={{ block: place.block }} onChoose={onChoose}>
            Block <span className="mono">{place.block}</span>
          </Choose>
        );
    }
  };
  return (
    <Panel
      label="What changed"
      title="What changed"
      belongs={`${before.name} → ${after.name}`}
      lines={
        <StatusLine tone={changes.length > 0 ? 'changed' : 'quiet'} word={`${changes.length} changes`}>
          each under the place it is in, to approve one by one
        </StatusLine>
      }
      {...fold}
    >
      {changes.length === 0 && <p className="muted">Nothing: the two definitions compile to the same flow</p>}
      <ol className="changes">
        {groups.map((group, i) => (
          <li key={i} className="change-group">
            <PanelPart label={head(group.place)}>
              <ul>
                {group.changes.map((change, j) => (
                  <ListRow key={j}>
                    <ChangeLine change={change} />
                  </ListRow>
                ))}
              </ul>
            </PanelPart>
          </li>
        ))}
      </ol>
    </Panel>
  );
}

/// The faults of a definition, each with its file and field: in place of the chart, or of the list of changes when
/// it is the definition compared against that does not compile.
export function Faults({ faults }: { faults: Fault[] }) {
  return (
    <table className="table faults">
      <thead>
        <tr>
          <th scope="col" className="label">
            File
          </th>
          <th scope="col" className="label">
            Field
          </th>
          <th scope="col" className="label">
            What is wrong
          </th>
        </tr>
      </thead>
      <tbody>
        {faults.map((fault, i) => (
          <tr key={i}>
            <td className="mono">{fault.file}</td>
            <td>{fault.field ? <span className="mono">{fault.field}</span> : <span className="muted">the whole file</span>}</td>
            <td>{faultWords(fault)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// A list of cells, each chosen by its name, with where it stands from the cell `from`.
function Cells({ flow, from, ids, onChoose }: { flow: Flow; from?: string; ids: string[]; onChoose: (c: Chosen) => void }) {
  return (
    <>
      {ids.map((id, i) => {
        const at = from ? where(flow, from, id) : null;
        return (
          <span key={id}>
            {i > 0 && ', '}
            <Choose chosen={{ cell: id }} onChoose={onChoose}>
              <span className="mono">{id}</span>
            </Choose>
            {at === 'itself' && <span className="muted"> (itself)</span>}
          </span>
        );
      })}
    </>
  );
}

/// The changes of one place of the flow, in the panel of that place.
function ChangesHere({ changes }: { changes: Change[] }) {
  if (changes.length === 0) return null;
  return (
    <PanelPart label="What changed" count={changes.length}>
      <ul className="changes-here">
        {changes.map((change, i) => (
          <ListRow key={i}>
            <ChangeLine change={change} />
          </ListRow>
        ))}
      </ul>
    </PanelPart>
  );
}

/// Everything about a cell: what its agent is told to do or the engine's act that carries it out, what it reads and
/// who writes each, what it may write and who reads each, when it runs, what it waits for, its limits and the cells it
/// shares a session with.
export function CellPanel(props: {
  column: CellColumn;
  matrix: Matrix;
  flow: Flow;
  changes: Change[];
  onChoose: (c: Chosen) => void;
  fold: Fold;
}) {
  const { column, flow, changes, onChoose, fold } = props;
  const cell = (column.cell ?? column.other) as Cell;
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const sharing = cell.line ? (flow.lines.find((l) => l.name === cell.line)?.cells ?? []).filter((c) => c !== cell.id) : [];
  const later = (ids: string[]) => ids.filter((id) => where(flow, cell.id, id) === 'below');
  return (
    <Panel
      label={`Cell ${cell.id}`}
      id={cell.id}
      title={cell.title}
      belongs={
        cell.kind === 'agent' ? (
          <>
            <span className="line-name">{runBy(cell)}</span> line
          </>
        ) : (
          'engine'
        )
      }
      lines={
        <>
          <ChangeStatus status={column.status} changes={changes.length} />
          <StatusLine word={cell.condition ? 'Runs when' : 'Runs always'}>
            {cell.condition?.on === 'block' ? (
              <>
                <Choose chosen={{ block: cell.condition.block }} onChoose={onChoose}>
                  <span className="mono">{cell.condition.block}</span>
                </Choose>{' '}
                is <span className="mono">{cell.condition.value}</span>
              </>
            ) : cell.condition ? (
              conditionWords(cell.condition)
            ) : undefined}
          </StatusLine>
          {cell.preconditions.length > 0 && (
            <StatusLine word="Waits until">{cell.preconditions.map(testWords).join('; ')}</StatusLine>
          )}
          <OwnLimits cell={cell} flow={flow} />
        </>
      }
      {...fold}
    >
      {!column.cell && <p className="muted">Not in the version shown: this is the cell as the other version has it</p>}
      <PanelPart label="Reads" count={cell.inputs.length}>
        <ul className="uses">
          {cell.inputs.map((name, place) => {
            const block = blocks.get(name) as Block;
            const back = later(block.writtenBy.cells.filter((id) => id !== cell.id));
            const besides = writersBesideCells(block);
            const writers = block.writtenBy.cells;
            const taken = cell.input[name];
            return (
              <ListRow key={name}>
                <span className="muted mono" title="Its place among what the cell reads, in the order the cell is given them">
                  {place + 1}
                </span>
                <Choose chosen={{ block: name }} onChoose={onChoose}>
                  <span className="mono">{name}</span>
                </Choose>
                {taken?.kind && <span className="muted">{inputKindWords(taken.kind)}</span>}
                <span>
                  <span className="muted">written by </span>
                  {writers.length > 0 && <Cells flow={flow} from={cell.id} ids={writers} onChoose={onChoose} />}
                  {writers.length > 0 && besides.length > 0 && ', '}
                  {besides.join(', ')}
                </span>
                {back.length > 0 && <span className="way-back">way back from {back.join(', ')}</span>}
                {block.what !== null && <span className="row-note">{block.what}</span>}
                {taken?.why && <span className="row-note">{taken.why}</span>}
                {thereWhen(flow, cell.id, block) !== '' && (
                  <span className="row-note muted">there {thereWhen(flow, cell.id, block)}</span>
                )}
              </ListRow>
            );
          })}
        </ul>
      </PanelPart>

      <PanelPart label="May write" count={cell.outputs.length}>
        <ul className="uses">
          {cell.outputs.map((name) => {
            const block = blocks.get(name) as Block;
            const above = block.readBy.cells.filter((id) => where(flow, cell.id, id) === 'above');
            const aboveConditions = block.readBy.conditions.filter((id) => where(flow, cell.id, id) === 'above');
            const sendsBack = [...new Set([...above, ...aboveConditions])];
            return (
              <ListRow key={name}>
                <Choose chosen={{ block: name }} onChoose={onChoose}>
                  <span className="mono">{name}</span>
                </Choose>
                {cell.mustWrite.includes(name) && <span className="muted">written every time</span>}
                {cell.inputs.includes(name) && <span className="muted">also read: {BROUGHT_UP_TO_DATE}</span>}
                <span>
                  <Readers flow={flow} from={cell.id} block={block} onChoose={onChoose} />
                </span>
                {sendsBack.length > 0 && <span className="way-back">way back to {sendsBack.join(', ')}</span>}
                {block.what !== null && <span className="row-note">{block.what}</span>}
              </ListRow>
            );
          })}
        </ul>
      </PanelPart>

      {cell.task !== null && (
        <PanelPart label="Task">
          <p className="content">{cell.task}</p>
        </PanelPart>
      )}
      {cell.act !== null && (
        <PanelPart label="Act" last>
          <p className="mono">{cell.act}</p>
        </PanelPart>
      )}
      {cell.kind === 'agent' && (
        <PanelPart label="Session" last>
          <p>
            {sharing.length > 0 ? (
              <>
                <span className="muted">Shares its session with </span>
                <Cells flow={flow} ids={sharing} onChoose={onChoose} />
              </>
            ) : (
              <span className="muted">Alone in its session</span>
            )}
          </p>
        </PanelPart>
      )}

      <ChangesHere changes={changes} />
    </Panel>
  );
}

/// The limits a cell sets of its own, where they differ from the flow's, which the view's line says once.
function OwnLimits({ cell, flow }: { cell: Cell; flow: Flow }) {
  const own = (['runs', 'returns', 'calls'] as const).filter((l) => cell.limits[l] !== flow.limits[l]);
  if (own.length === 0) return null;
  return (
    <StatusLine word="Limits of its own">
      {own.map((l) => `${cell.limits[l]} ${LIMIT_WORDS[l]} (the flow's: ${flow.limits[l]})`).join(', ')}
    </StatusLine>
  );
}

/// Who reads a block, in words: after its name (`read by plan`), or alone, after a label that says it (`plan`). The
/// named tests that read it are named, each with its words on the pointer's tip.
function Readers(props: { flow: Flow; from?: string; block: Block; alone?: boolean; onChoose: (c: Chosen) => void }) {
  const { flow, from, block, alone, onChoose } = props;
  const { cells, conditions, tests, shown } = block.readBy;
  if (cells.length === 0 && conditions.length === 0 && tests.length === 0 && !shown) return null;
  const before = cells.length + conditions.length;
  const words = (name: string) => {
    const test = flow.tests.find((t) => t.name === name);
    return test ? testWords(test) : undefined;
  };
  return (
    <>
      {cells.length > 0 && (
        <>
          {!alone && <span className="muted">read by </span>}
          <Cells flow={flow} from={from} ids={cells} onChoose={onChoose} />
        </>
      )}
      {conditions.length > 0 && (
        <>
          <span className="muted">{cells.length > 0 ? '; ' : ''}decides whether </span>
          <Cells flow={flow} from={from} ids={conditions} onChoose={onChoose} />
          <span className="muted"> run{conditions.length === 1 ? 's' : ''}</span>
        </>
      )}
      {tests.length > 0 && (
        <>
          <span className="muted">
            {before > 0 ? '; ' : !alone ? 'read by ' : ''}the test{tests.length === 1 ? '' : 's'}{' '}
          </span>
          {tests.map((name, i) => (
            <span key={name}>
              {i > 0 && ', '}
              <span className="mono" title={words(name)}>
                {name}
              </span>
            </span>
          ))}
        </>
      )}
      {shown && <span className="muted">{before + tests.length > 0 ? '; ' : ''}shown</span>}
    </>
  );
}

/// Everything about a block: its values, who writes it, who reads it, and the ways back through it.
export function BlockPanel(props: {
  row: BlockRow;
  matrix: Matrix;
  flow: Flow;
  changes: Change[];
  onChoose: (c: Chosen) => void;
  fold: Fold;
}) {
  const { row, flow, changes, onChoose, fold } = props;
  const block = (row.block ?? row.other) as Block;
  const besides = writersBesideCells(block);
  const backs = flow.cells.flatMap((reader, r) => {
    const mark = markOf(reader, block.name);
    if (!mark.read && !mark.condition) return [];
    const writers = flow.cells.filter((w, i) => i > r && markOf(w, block.name).write).map((w) => w.id);
    return writers.length > 0 ? [{ reader: reader.id, writers, byCondition: !mark.read }] : [];
  });
  return (
    <Panel
      label={`Block ${block.name}`}
      title={<span className="mono">{block.name}</span>}
      belongs={block.values ? `a choice of ${block.values.join(', ')}` : 'block'}
      lines={
        <>
          <ChangeStatus status={row.status} changes={changes.length} />
          {block.what !== null && <StatusLine word="What it is">{block.what}</StatusLine>}
          <StatusLine word="Written by">
            {block.writtenBy.cells.length > 0 && <Cells flow={flow} ids={block.writtenBy.cells} onChoose={onChoose} />}
            {block.writtenBy.cells.length > 0 && besides.length > 0 && ', '}
            {besides.join(', ')}
          </StatusLine>
          <StatusLine word="Read by">
            <Readers flow={flow} block={block} alone onChoose={onChoose} />
          </StatusLine>
        </>
      }
      {...fold}
    >
      {!row.block && <p className="muted">Not in the version shown: this is the block as the other version has it</p>}
      {backs.length === 0 && changes.length === 0 && <p className="muted">No way back runs through it</p>}
      {backs.length > 0 && (
        <PanelPart label="Ways back" count={backs.length} last={changes.length > 0}>
          <ul className="uses">
            {backs.map(({ reader, writers, byCondition }) => (
              <ListRow key={reader}>
                <span className="way-back mono">
                  {writers.join(', ')} → {reader}
                </span>
                <span className="muted">
                  {byCondition
                    ? `a write by ${writers.join(' or ')} decides again whether ${reader} runs`
                    : `a write by ${writers.join(' or ')} runs ${reader} again`}
                </span>
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
      <ChangesHere changes={changes} />
    </Panel>
  );
}
