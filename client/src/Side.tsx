// The side of the chart: everything about the cell or block the owner chose, or, when nothing is chosen and there is a
// definition to compare against, the list of what changed, each change as the owner approves it.

import { XIcon } from '@primer/octicons-react';
import { CounterLabel, IconButton } from '@primer/react';
import type { ReactNode } from 'react';

import type { Block, Cell, Change, Fault, Flow } from './api';
import { type Chosen, RunBy, StatusLabel } from './Chart';
import { type Column, type Matrix, type Row, markOf, where } from './matrix';
import {
  type Place,
  LIMIT_WORDS,
  changeWords,
  conditionWords,
  faultWords,
  placeOf,
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

/// One change: what changed, what it was, and what it is.
export function ChangeLine({ change }: { change: Change }) {
  const { what, before, after } = changeWords(change);
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
}) {
  const { changes, after, before, onChoose } = props;
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
            Test <code>{place.test}</code>
          </>
        );
      case 'cell':
        return (
          <Choose chosen={{ cell: place.cell }} onChoose={onChoose}>
            <code>{place.cell}</code> {title(place.cell)}
          </Choose>
        );
      case 'block':
        return (
          <Choose chosen={{ block: place.block }} onChoose={onChoose}>
            Block <code>{place.block}</code>
          </Choose>
        );
    }
  };
  return (
    <section className="side-section" aria-label="What changed">
      <h2 className="side-title">
        What changed <CounterLabel>{changes.length}</CounterLabel>
      </h2>
      {changes.length === 0 && <p className="quiet">Nothing: the two definitions compile to the same flow.</p>}
      <ol className="changes">
        {groups.map((group, i) => (
          <li key={i} className="change-group">
            <h3>{head(group.place)}</h3>
            <ul>
              {group.changes.map((change, j) => (
                <li key={j}>
                  <ChangeLine change={change} />
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ol>
    </section>
  );
}

/// The faults of a definition, each with its file and field: in place of the chart, or of the list of changes when
/// it is the definition compared against that does not compile.
export function Faults({ faults }: { faults: Fault[] }) {
  return (
    <table className="faults">
      <thead>
        <tr>
          <th scope="col">File</th>
          <th scope="col">Field</th>
          <th scope="col">What is wrong</th>
        </tr>
      </thead>
      <tbody>
        {faults.map((fault, i) => (
          <tr key={i}>
            <td>
              <code>{fault.file}</code>
            </td>
            <td>{fault.field ? <code>{fault.field}</code> : <span className="quiet">the whole file</span>}</td>
            <td>{faultWords(fault)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function PanelHead(props: { kind: string; name: string; title?: string; status: Row['status']; onClose: () => void }) {
  return (
    <header className="panel-head">
      <div>
        <span className="quiet">{props.kind}</span> <code className="panel-id">{props.name}</code>{' '}
        <StatusLabel status={props.status} />
        {props.title && <h2 className="side-title">{props.title}</h2>}
      </div>
      <IconButton icon={XIcon} aria-label="Close" variant="invisible" size="small" onClick={props.onClose} />
    </header>
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
              {id}
            </Choose>
            {at === 'itself' && <span className="quiet"> (itself)</span>}
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
    <section className="side-section">
      <h3>What changed</h3>
      <ul className="changes-here">
        {changes.map((change, i) => (
          <li key={i}>
            <ChangeLine change={change} />
          </li>
        ))}
      </ul>
    </section>
  );
}

/// Everything about a cell: what it reads and who writes each, what it may write and who reads each, when it runs,
/// what it waits for, its limits and the cells it shares a session with.
export function CellPanel(props: {
  row: Row;
  matrix: Matrix;
  flow: Flow;
  changes: Change[];
  onChoose: (c: Chosen) => void;
}) {
  const { row, flow, changes, onChoose } = props;
  const cell = (row.cell ?? row.other) as Cell;
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const sharing = cell.line ? (flow.lines.find((l) => l.name === cell.line)?.cells ?? []).filter((c) => c !== cell.id) : [];
  const later = (ids: string[]) => ids.filter((id) => where(flow, cell.id, id) === 'below');
  return (
    <section className="panel" aria-label={`Cell ${cell.id}`}>
      <PanelHead kind="Cell" name={cell.id} title={cell.title} status={row.status} onClose={() => onChoose(null)} />
      {!row.cell && <p className="quiet">Not in the version shown: this is the cell as the other version has it.</p>}
      <dl className="facts">
        <dt>Run by</dt>
        <dd>
          <RunBy cell={cell} />
          {cell.kind === 'agent' && (
            <span className="quiet">
              {' '}
              line{sharing.length > 0 ? ', sharing its session with ' : ', alone in its session'}
            </span>
          )}
          {sharing.length > 0 && <Cells flow={flow} ids={sharing} onChoose={onChoose} />}
        </dd>
        <dt>Runs when</dt>
        <dd>
          {cell.condition?.on === 'block' ? (
            <>
              <Choose chosen={{ block: cell.condition.block }} onChoose={onChoose}>
                {cell.condition.block}
              </Choose>{' '}
              is {cell.condition.value}
            </>
          ) : (
            conditionWords(cell.condition)
          )}
        </dd>
        <dt>Waits until</dt>
        <dd>
          {cell.preconditions.length === 0 ? (
            <span className="quiet">it does not wait</span>
          ) : (
            cell.preconditions.map((p) => <div key={p.name}>{p.words}</div>)
          )}
        </dd>
      </dl>

      <section className="side-section">
        <h3>Reads</h3>
        {cell.inputs.length === 0 && <p className="quiet">nothing</p>}
        <ul className="uses">
          {cell.inputs.map((name) => {
            const block = blocks.get(name) as Block;
            const back = later(block.writtenBy.cells.filter((id) => id !== cell.id));
            const besides = writersBesideCells(block);
            const writers = block.writtenBy.cells;
            return (
              <li key={name}>
                <Choose chosen={{ block: name }} onChoose={onChoose}>
                  <code>{name}</code>
                </Choose>
                <span className="quiet"> written by </span>
                {writers.length > 0 && <Cells flow={flow} from={cell.id} ids={writers} onChoose={onChoose} />}
                {writers.length > 0 && besides.length > 0 && ', '}
                {besides.join(', ')}
                {back.length > 0 && <span className="way-back"> way back from {back.join(', ')}</span>}
              </li>
            );
          })}
        </ul>
      </section>

      <section className="side-section">
        <h3>May write</h3>
        {cell.outputs.length === 0 && <p className="quiet">nothing</p>}
        <ul className="uses">
          {cell.outputs.map((name) => {
            const block = blocks.get(name) as Block;
            const above = block.readBy.cells.filter((id) => where(flow, cell.id, id) === 'above');
            const aboveConditions = block.readBy.conditions.filter((id) => where(flow, cell.id, id) === 'above');
            const sendsBack = [...new Set([...above, ...aboveConditions])];
            return (
              <li key={name}>
                <Choose chosen={{ block: name }} onChoose={onChoose}>
                  <code>{name}</code>
                </Choose>
                <Readers flow={flow} from={cell.id} block={block} onChoose={onChoose} />
                {sendsBack.length > 0 && <span className="way-back"> way back to {sendsBack.join(', ')}</span>}
              </li>
            );
          })}
        </ul>
      </section>

      <section className="side-section">
        <h3>Limits</h3>
        <ul className="limits">
          {(['runs', 'returns', 'calls'] as const).map((limit) => (
            <li key={limit}>
              <strong>{cell.limits[limit]}</strong> {LIMIT_WORDS[limit]}
              {cell.limits[limit] === flow.limits[limit] ? (
                <span className="quiet"> (the flow's)</span>
              ) : (
                <span className="quiet"> (its own; the flow's is {flow.limits[limit]})</span>
              )}
            </li>
          ))}
        </ul>
      </section>
      <ChangesHere changes={changes} />
    </section>
  );
}

/// Who reads a block, in words: after its name (`read by plan`), or alone, after a heading that says it (`plan`).
function Readers(props: { flow: Flow; from?: string; block: Block; alone?: boolean; onChoose: (c: Chosen) => void }) {
  const { flow, from, block, alone, onChoose } = props;
  const { cells, conditions, shown } = block.readBy;
  if (cells.length === 0 && conditions.length === 0 && !shown) return null;
  return (
    <>
      {cells.length > 0 && (
        <>
          {!alone && <span className="quiet"> read by </span>}
          <Cells flow={flow} from={from} ids={cells} onChoose={onChoose} />
        </>
      )}
      {conditions.length > 0 && (
        <>
          <span className="quiet">{cells.length > 0 ? '; ' : ' '}decides whether </span>
          <Cells flow={flow} from={from} ids={conditions} onChoose={onChoose} />
          <span className="quiet"> run{conditions.length === 1 ? 's' : ''}</span>
        </>
      )}
      {shown && <span className="quiet">{cells.length + conditions.length > 0 ? '; ' : ' '}shown</span>}
    </>
  );
}

/// Everything about a block: its values, who writes it, who reads it, and the ways back through it.
export function BlockPanel(props: {
  column: Column;
  matrix: Matrix;
  flow: Flow;
  changes: Change[];
  onChoose: (c: Chosen) => void;
}) {
  const { column, flow, changes, onChoose } = props;
  const block = (column.block ?? column.other) as Block;
  const besides = writersBesideCells(block);
  const backs = flow.cells.flatMap((reader, r) => {
    const mark = markOf(reader, block.name);
    if (!mark.read && !mark.condition) return [];
    const writers = flow.cells.filter((w, i) => i > r && markOf(w, block.name).write).map((w) => w.id);
    return writers.length > 0 ? [{ reader: reader.id, writers, byCondition: !mark.read }] : [];
  });
  return (
    <section className="panel" aria-label={`Block ${block.name}`}>
      <PanelHead kind="Block" name={block.name} status={column.status} onClose={() => onChoose(null)} />
      {!column.block && <p className="quiet">Not in the version shown: this is the block as the other version has it.</p>}
      <dl className="facts">
        <dt>Values</dt>
        <dd>{block.values ? `a choice of ${block.values.join(', ')}` : <span className="quiet">not a choice</span>}</dd>
        <dt>Written by</dt>
        <dd>
          {block.writtenBy.cells.length > 0 && <Cells flow={flow} ids={block.writtenBy.cells} onChoose={onChoose} />}
          {block.writtenBy.cells.length > 0 && besides.length > 0 && ', '}
          {besides.join(', ')}
        </dd>
        <dt>Read by</dt>
        <dd>
          <Readers flow={flow} block={block} alone onChoose={onChoose} />
        </dd>
      </dl>
      {backs.length > 0 && (
        <section className="side-section">
          <h3>Ways back</h3>
          <ul className="uses">
            {backs.map(({ reader, writers, byCondition }) => (
              <li key={reader}>
                <span className="way-back">
                  {writers.join(', ')} → {reader}
                </span>
                <span className="quiet">
                  {byCondition
                    ? `: a write by ${writers.join(' or ')} decides again whether ${reader} runs`
                    : `: a write by ${writers.join(' or ')} runs ${reader} again`}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <ChangesHere changes={changes} />
    </section>
  );
}
