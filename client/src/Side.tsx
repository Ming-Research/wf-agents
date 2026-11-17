// The panels of the flow view, shown in its window over the chart's right: everything about the cell, block, line,
// named test, tool or group the owner chose, or about what `[every_agent]` names.

import { Fragment, type ReactNode, useState } from 'react';

import type {
  ActWords,
  Block,
  Cell,
  Condition,
  Fault,
  Flow,
  SentCarried,
  SentPart,
  SentPiece,
  Unknown,
} from './api';
import type { Chosen } from './Chart';
import { FormTree } from './FormTree';
import { type ConditionRow, type LineRow, type Matrix, canRun, kindOf, namesItself, where } from './matrix';
import { Arrow, Attr, ListRow, Panel, PanelPart, StatusLine } from './parts';
import {
  charactersWords,
  conditionWords,
  faultWords,
  inputKindWords,
  lineKeys,
  listed,
  readsWords,
  runBy,
  snake,
  thereWhen,
  writersBesideCells,
} from './words';

/// Whether the panel is folded away, and the way to fold it or bring it back.
export type Fold = { folded: boolean; onFold: () => void; edge?: ReactNode };

/// What the named tests among these keys say, each as its file writes it; none where no key is a test.
function testsSay(keys: Condition[]): string | undefined {
  const says = keys.flatMap((k) => (k.on === 'test' ? [k.test.words] : []));
  return says.length > 0 ? says.join('; ') : undefined;
}

/// What a name a cell takes is chosen by: its datum, or its group.
const chosenOf = (n: { name: string; blocks: string[] }): Chosen => (namesItself(n) ? { block: n.name } : { group: n.name });

/// The kind a cell is given each block of a name in, as `[kinds]` writes it: of a datum, `kind` and its kind; of a
/// group, `kinds` and each block with its kind; nothing for a cell given none, an engine's.
function Kinds({ cell, taken }: { cell: Cell; taken: { name: string; blocks: string[] } }) {
  const kind = (b: string) => inputKindWords(kindOf(cell, b));
  if (namesItself(taken)) {
    return kind(taken.name) ? <Attr name="kind" value={kind(taken.name)} /> : null;
  }
  const kinds = taken.blocks.filter((b) => kind(b) !== '');
  return kinds.length > 0 ? <Attr name="kinds" value={listed(kinds.map((b) => `${b} ${kind(b)}`))} /> : null;
}

/// The row of what a cell reads by `[every_agent]` in the reads of its panel: the key, chosen to open its panel, and the
/// names it reads so, as `flow.toml` writes them; none where it reads nothing so.
function EveryAgentRow(props: { cell: Cell; flow: Flow; onChoose: (c: Chosen) => void }) {
  const { cell, flow, onChoose } = props;
  const alike = new Set(cell.given.filter((g) => g.list === 'every_agent').map((g) => g.block));
  const names = flow.everyAgent.reads.filter((n) => n.blocks.some((b) => alike.has(b))).map((n) => n.name);
  if (names.length === 0) return null;
  return (
    <ListRow>
      <Choose chosen={{ everyAgent: true }} onChoose={onChoose}>
        <span className="mono">every_agent</span>
      </Choose>
      <Attr name="reads" value={names.join(', ')} />
    </ListRow>
  );
}

/// A name the owner can choose to see it on the chart and in the panel.
function Choose({
  chosen,
  onChoose,
  children,
}: {
  chosen: Chosen;
  onChoose: (c: Chosen) => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className="link" onClick={() => onChoose(chosen)}>
      {children}
    </button>
  );
}

/// The faults of a definition, each with its file and field, in place of the chart.
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
            <td>
              {fault.field ? (
                <span className="mono">{fault.field}</span>
              ) : (
                <span className="muted">the whole file</span>
              )}
            </td>
            <td>{faultWords(fault)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// A list of cells, each chosen by its name, with where it stands from the cell `from`.
function Cells({
  flow,
  from,
  ids,
  onChoose,
}: {
  flow: Flow;
  from?: string;
  ids: string[];
  onChoose: (c: Chosen) => void;
}) {
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

/// What the owner opens and folds away again where it stands.
function Folded({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="opener">
      <button type="button" className="link" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? 'Hide' : 'Show'} {label}
      </button>
      {open && children}
    </div>
  );
}

/// A text as it is sent: what is written as written, and each place filled in a run by its name, as `{name}`.
function SentText({ pieces }: { pieces: SentPiece[] }) {
  return (
    <pre className="content">
      {pieces.map((piece, i) =>
        piece.piece === 'text' ? piece.text : <span key={i} className="muted">{`{${piece.name}}`}</span>,
      )}
    </pre>
  );
}

/// The text of pieces sent as written, with each place filled in a run as `{name}`.
function textOf(pieces: SentPiece[]): string {
  return pieces.map((p) => (p.piece === 'text' ? p.text : `{${p.name}}`)).join('');
}

/// The size of a text sent as written, where all of it is.
function sizeOf(part: SentPart): number {
  return part.pieces.reduce((n, piece) => n + (piece.piece === 'text' ? piece.text.length : 0), 0);
}

/// Everything an agent's cell is sent, numbered in the order sent, in three sections by the way it reaches the agent:
/// the program's own with the form, which the program makes a tool of, in the form's three parts, each drawn as a
/// tree; the system prompt; the message. Each part shows its key in the files, its attributes, the file its words come
/// from, and its text in each form it is sent in: as written, `whole`, `held`, `held_new`, and each item of a list by
/// its key; a text every cell of the line is sent alike, or a long one, is folded.
function AgentSent({
  parts,
  line,
  cells,
  unknown,
  taskFile,
  onChoose,
}: {
  parts: SentPart[];
  line: string;
  cells: string[];
  unknown: Unknown[];
  /// The file the cell's task is written in, where it is not written in place.
  taskFile: string | null;
  onChoose: (c: Chosen) => void;
}) {
  const row = (part: SentPart) => {
    const place = parts.indexOf(part);
    const fold = part.carried !== 'message' || part.what === 'task' || part.what === 'outputs';
    return (
      <ListRow key={`${part.what} ${part.name}`}>
        <span className="muted mono">{place + 1}</span>
        {part.what === 'input' ? (
          <Choose chosen={{ block: part.name }} onChoose={onChoose}>
            <span className="mono">{part.name}</span>
          </Choose>
        ) : (
          <span className="mono">{part.name}</span>
        )}
        {part.only && <Attr name="only" value={part.only} />}
        {part.what === 'task' && taskFile && <Attr name="file" value={taskFile} />}
        {part.kind && <Attr name="kind" value={part.kind} />}
        <span className="muted mono">{part.source}</span>
        {part.pieces.length === 0 ? null : fold ? (
          <>
            <span className="muted">{charactersWords(sizeOf(part))}</span>
            <Folded label="the text">
              {part.what === 'form' ? (
                <FormTree text={textOf(part.pieces)} part={part.name} />
              ) : (
                <SentText pieces={part.pieces} />
              )}
            </Folded>
          </>
        ) : (
          <div className="row-note">
            <SentText pieces={part.pieces} />
          </div>
        )}
        {part.whole.length > 0 && (
          <div className="row-note">
            <Attr name="whole" value="" />
            <SentText pieces={part.whole} />
          </div>
        )}
        {part.held.length > 0 && (
          <div className="row-note">
            <Attr name="held" value="" />
            <SentText pieces={part.held} />
          </div>
        )}
        {part.heldNew.length > 0 && (
          <div className="row-note">
            <Attr name="held_new" value="" />
            <SentText pieces={part.heldNew} />
          </div>
        )}
        {part.items.map(([key, pieces]) => (
          <div className="row-note" key={key}>
            <Attr name={key} value="" />
            <SentText pieces={pieces} />
          </div>
        ))}
        {part.after.length > 0 && (
          <div className="row-note">
            <Attr name="after" value="" />
            <SentText pieces={part.after} />
          </div>
        )}
      </ListRow>
    );
  };
  const of = (...carried: SentCarried[]) => parts.filter((p) => carried.includes(p.carried));
  return (
    <>
      {unknown.length > 0 && (
        <PanelPart label="Unknown names" count={unknown.length}>
          <ul className="uses">
            {unknown.map((u) => (
              <ListRow key={`${u.name} ${u.source}`}>
                <span className="mono">{u.name}</span>
                <span className="muted mono">{u.source}</span>
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
      <PanelPart label="program" count={of('program', 'call').length}>
        <ul className="uses">{of('program', 'call').map(row)}</ul>
      </PanelPart>
      <PanelPart label="System prompt" count={of('system').length}>
        <ul className="uses">
          <ListRow>
            <Attr name="line" value={line} />
            <Attr name="cells" value={cells.join(', ')} />
          </ListRow>
          {of('system').map(row)}
        </ul>
      </PanelPart>
      <PanelPart label="Message" count={of('message').length}>
        <ul className="uses">{of('message').map(row)}</ul>
      </PanelPart>
    </>
  );
}

/// Everything about a cell: what its agent is told to do or the engine's act that carries it out, what runs it and
/// what it is given and who writes each, what it may write and who reads each, the tools of its own it may ask the
/// engine for where the flow names none for every agent's cell, what its agent is sent as it is entered, when it runs,
/// what it waits for, its limits and the cells it shares a session with.
export function CellPanel(props: {
  cell: Cell;
  matrix: Matrix;
  flow: Flow;
  /// What the cell's agent is sent as the cell is entered, part by part in the order sent; none for an engine's
  /// cell, and where the page was not given it.
  sent?: SentPart[];
  /// What an engine's cell's act writes; none for an agent's cell, and where the page was not given it.
  act?: ActWords;
  /// The names a text sent to an agent gives that nothing defines.
  unknown?: Unknown[];
  onChoose: (c: Chosen) => void;
}) {
  const { cell, flow, sent, act, unknown, onChoose } = props;
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const onLine = cell.line ? (flow.lines.find((l) => l.name === cell.line)?.cells ?? []) : [];
  const sharing = onLine.filter((c) => c !== cell.id);
  const cellOf = (id: string) => flow.cells.find((c) => c.id === id);
  // The cells after this one whose write can run it: a write of theirs sends the work back to it.
  const later = (ids: string[]) =>
    ids.filter((id) => {
      const writer = cellOf(id);
      return where(flow, cell.id, id) === 'below' && writer !== undefined && canRun(writer, cell);
    });
  // What its own file watches, and whether it reads anything by `[every_agent]`, which one row of the reads says.
  const own = cell.watches;
  const alikeRead = cell.given.some((g) => g.list === 'every_agent' && !cell.watches.some((w) => w.blocks.includes(g.block)));
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
          {(['when', 'requires'] as const).map(
            (key) =>
              cell[key].length > 0 && (
                <StatusLine key={key} word={<Attr name={key} value={conditionWords(cell[key])} />}>
                  {testsSay(cell[key])}
                </StatusLine>
              ),
          )}
          {cell.takesMain && <StatusLine word={<Attr name="takes_main" value="true" />} />}
          <OwnLimits cell={cell} flow={flow} />
        </>
      }
    >
      {sent && sent.length > 0 ? (
        <>
          <PanelPart label="watches" count={own.length}>
            <ul className="uses">
              {own.map((w) => (
                <ListRow key={w.name}>
                  <Choose chosen={chosenOf(w)} onChoose={onChoose}>
                    <span className="mono">{w.name}</span>
                  </Choose>
                  <Kinds cell={cell} taken={w} />
                  {w.do && (
                    <div className="row-note">
                      <Attr name="do" value="" />
                      {w.do}
                    </div>
                  )}
                </ListRow>
              ))}
            </ul>
          </PanelPart>
          <AgentSent
            parts={sent}
            line={cell.line ?? ''}
            cells={onLine}
            unknown={unknown ?? []}
            taskFile={cell.taskFile}
            onChoose={onChoose}
          />
        </>
      ) : (
        <>
          <PanelPart label="watches" count={own.length}>
            <ul className="uses">
              {own.map((w, place) => {
                // Of a group, there is no one block: its group is chosen by its name.
                const block = namesItself(w) ? blocks.get(w.name) : undefined;
                // The later cells that write it, or one of its group's, send the work back to this one.
                const writers = w.blocks.flatMap((b) => blocks.get(b)?.writtenBy.cells ?? []);
                const back = later([...new Set(writers)].filter((id) => id !== cell.id));
                return (
                  <ListRow key={w.name}>
                    <span
                      className="muted mono"
                      title="Its place among what the cell watches, in the order the cell names them"
                    >
                      {place + 1}
                    </span>
                    <Choose chosen={chosenOf(w)} onChoose={onChoose}>
                      <span className="mono">{w.name}</span>
                    </Choose>
                    {!namesItself(w) && <Attr name="group" value={listed(w.blocks)} />}
                    <Kinds cell={cell} taken={w} />
                    {block && <WrittenBy flow={flow} cell={cell} block={block} onChoose={onChoose} />}
                    {back.length > 0 && <Attr name="way_back" value={listed(back)} />}
                    {block && block.what !== null && <span className="row-note">{block.what}</span>}
                    {w.do && (
                      <div className="row-note">
                        <Attr name="do" value="" />
                        {w.do}
                      </div>
                    )}
                  </ListRow>
                );
              })}
            </ul>
          </PanelPart>

          {cell.kind === 'agent' && (cell.reads.length > 0 || alikeRead) && (
            <PanelPart label="reads" count={cell.reads.length + (alikeRead ? 1 : 0)}>
              <ul className="uses">
                <EveryAgentRow cell={cell} flow={flow} onChoose={onChoose} />
                {cell.reads.map((r, place) => {
                  const block = namesItself(r) ? blocks.get(r.name) : undefined;
                  return (
                    <ListRow key={r.name}>
                      <span
                        className="muted mono"
                        title="Its place among what the cell reads, in the order the cell names them"
                      >
                        {place + 1}
                      </span>
                      <Choose chosen={chosenOf(r)} onChoose={onChoose}>
                        <span className="mono">{r.name}</span>
                      </Choose>
                      {!namesItself(r) && <Attr name="group" value={listed(r.blocks)} />}
                      <Kinds cell={cell} taken={r} />
                      {block && <WrittenBy flow={flow} cell={cell} block={block} onChoose={onChoose} />}
                      {block && block.what !== null && <span className="row-note">{block.what}</span>}
                      {block && thereWhen(flow, cell.id, block) !== '' && (
                        <span className="row-note">
                          <Attr name="written_before" value={thereWhen(flow, cell.id, block)} />
                        </span>
                      )}
                    </ListRow>
                  );
                })}
              </ul>
            </PanelPart>
          )}

          <PanelPart label="outputs" count={cell.outputs.length}>
            <ul className="uses">
              {cell.outputs.map((name) => {
                const block = blocks.get(name) as Block;
                // A write of it sends the work back to each cell before this one that watches it, by itself or
                // through a group, and that a write of this one's can run.
                const sendsBack = flow.cells
                  .filter(
                    (c) =>
                      where(flow, cell.id, c.id) === 'above' &&
                      c.watches.some((w) => w.blocks.includes(name)) &&
                      canRun(cell, c),
                  )
                  .map((c) => c.id);
                return (
                  <ListRow key={name}>
                    <Choose chosen={{ block: name }} onChoose={onChoose}>
                      <span className="mono">{name}</span>
                    </Choose>
                    {cell.mustWrite.includes(name) && <Attr name="must_write" value="true" />}
                    {cell.given.some((g) => g.block === name) && <Attr name="given" value="true" />}
                    <span>
                      <Readers flow={flow} from={cell.id} block={block} onChoose={onChoose} />
                    </span>
                    {sendsBack.length > 0 && (
                      <Attr name="way_back" value={listed(sendsBack)} />
                    )}
                    {block.what !== null && <span className="row-note">{block.what}</span>}
                  </ListRow>
                );
              })}
            </ul>
          </PanelPart>

          {cell.kind === 'agent' && flow.tools.length === 0 && cell.tools.length > 0 && (
            <PanelPart label="tools" count={cell.tools.length}>
              <ul className="uses">
                {cell.tools.map((tool) => (
                  <ListRow key={tool}>
                    <span className="mono">{tool}</span>
                  </ListRow>
                ))}
              </ul>
            </PanelPart>
          )}

          {cell.task !== null && !(sent && sent.length > 0) && (
            <PanelPart label="task">
              {cell.taskFile && (
                <p>
                  <Attr name="file" value={cell.taskFile} />
                </p>
              )}
              <p className="content">{cell.task}</p>
            </PanelPart>
          )}
          {cell.act !== null && (
            <PanelPart label="act" last>
              <p>
                <span className="mono">{cell.act}</span> {act && <span className="muted mono">{act.source}</span>}
              </p>
              {act && (
                <ul className="uses">
                  {act.words.map(([key, pieces]) => (
                    <ListRow key={key}>
                      <span className="mono">{key}</span>
                      <div className="row-note">
                        <SentText pieces={pieces} />
                      </div>
                    </ListRow>
                  ))}
                </ul>
              )}
            </PanelPart>
          )}
          {cell.kind === 'agent' && (
            <PanelPart label="line" last>
              <p>
                <Attr name="line" value={cell.line} /> <span className="muted mono">cells</span>{' '}
                <Cells flow={flow} ids={[cell.id, ...sharing]} onChoose={onChoose} />
              </p>
            </PanelPart>
          )}
        </>
      )}
    </Panel>
  );
}

/// Who writes a block a cell names, the cells chosen by their names with where each stands from the cell.
function WrittenBy(props: { flow: Flow; cell: Cell; block: Block; onChoose: (c: Chosen) => void }) {
  const { flow, cell, block, onChoose } = props;
  const besides = writersBesideCells(block);
  const writers = block.writtenBy.cells;
  return (
    <span>
      <span className="muted mono">written_by </span>
      {writers.length > 0 && <Cells flow={flow} from={cell.id} ids={writers} onChoose={onChoose} />}
      {writers.length > 0 && besides.length > 0 && ', '}
      {besides.join(', ')}
    </span>
  );
}

/// The limits a cell sets of its own, where they differ from the flow's, which the view's line says once.
function OwnLimits({ cell, flow }: { cell: Cell; flow: Flow }) {
  const own = (['runs', 'returns', 'calls'] as const).filter((l) => cell.limits[l] !== flow.limits[l]);
  if (own.length === 0) return null;
  return (
    <StatusLine word="limits">
      {own.map((l) => (
        <Fragment key={l}>
          {' '}
          <Attr name={l} value={`${cell.limits[l]}`} />
        </Fragment>
      ))}
    </StatusLine>
  );
}

/// Who reads a block, in words: after its name (`read by plan`), or alone, after a label that says it (`plan`). The
/// named tests that read it are named, each with its words on the pointer's tip.
function Readers(props: { flow: Flow; from?: string; block: Block; alone?: boolean; onChoose: (c: Chosen) => void }) {
  const { flow, from, block, alone, onChoose } = props;
  const { cells, conditions, tests, shown } = block.readBy;
  if (cells.length === 0 && conditions.length === 0 && tests.length === 0 && !shown) return null;
  const words = (name: string) => {
    const test = flow.tests.find((t) => t.name === name);
    return test ? readsWords(test) : undefined;
  };
  return (
    <>
      {!alone && <span className="muted mono">read_by </span>}
      {cells.length > 0 && (
        <>
          <span className="muted mono">cells </span>
          <Cells flow={flow} from={from} ids={cells} onChoose={onChoose} />
        </>
      )}
      {conditions.length > 0 && (
        <>
          <span className="muted mono"> conditions </span>
          <Cells flow={flow} from={from} ids={conditions} onChoose={onChoose} />
        </>
      )}
      {tests.length > 0 && (
        <>
          <span className="muted mono"> tests </span>
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
      {shown && (
        <>
          {' '}
          <Attr name="shown" value="true" />
        </>
      )}
    </>
  );
}

/// Everything about a block: its values, who writes it, who reads it, and the ways back through it: each cell that
/// watches it, by itself, through a group or by `[every_agent]`, with the later cells whose write of it runs that cell
/// again, where their write can run it (`canRun`). A cell that only reads it, or whose condition reads it, is not run
/// by it, and no way back reaches it.
export function BlockPanel(props: {
  block: Block;
  matrix: Matrix;
  flow: Flow;
  onChoose: (c: Chosen) => void;
}) {
  const { block, flow, onChoose } = props;
  const besides = writersBesideCells(block);
  const backs = flow.cells.flatMap((reader, r) => {
    const watched = reader.watches.filter((w) => w.blocks.includes(block.name)).map((w) => w.name);
    if (watched.length === 0) return [];
    const writers = flow.cells
      .filter((w, i) => i > r && w.outputs.includes(block.name) && canRun(w, reader))
      .map((w) => w.id);
    return writers.length > 0 ? [{ reader: reader.id, writers, watched }] : [];
  });
  return (
    <Panel
      label={`Block ${block.name}`}
      title={<span className="mono">{block.name}</span>}
      belongs={
        block.values ? (
          <>
            <Attr name="values" value={listed(block.values)} />
            {block.default != null && (
              <>
                {' '}
                <Attr name="default" value={block.default} />
              </>
            )}
          </>
        ) : undefined
      }
      lines={
        <>
          {block.what !== null && <StatusLine word={<span className="mono">what</span>}>{block.what}</StatusLine>}
          <StatusLine word={<span className="mono">written_by</span>}>
            {block.writtenBy.cells.length > 0 && <Cells flow={flow} ids={block.writtenBy.cells} onChoose={onChoose} />}
            {block.writtenBy.cells.length > 0 && besides.length > 0 && ', '}
            {besides.join(', ')}
          </StatusLine>
          <StatusLine word={<span className="mono">read_by</span>}>
            <Readers flow={flow} block={block} alone onChoose={onChoose} />
          </StatusLine>
        </>
      }
    >
      {block.writes && block.writes.length > 0 && (
        <PanelPart label="writes" count={block.writes.length}>
          <ul className="uses">
            {block.writes.map(([key, pieces]) => (
              <ListRow key={key}>
                <span className="mono">{key}</span>
                <div className="row-note">
                  <SentText pieces={pieces} />
                </div>
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
      {backs.length === 0 && <p className="muted">No way back runs through it</p>}
      {backs.length > 0 && (
        <PanelPart label="way_back" count={backs.length}>
          <ul className="uses">
            {backs.map(({ reader, writers, watched }) => (
              <ListRow key={reader}>
                <span className="mono">
                  {writers.join(', ')} <Arrow to="right" />
                  <span className="visually-hidden">to</span> {reader}
                </span>
                <Attr name="watches" value={listed(watched)} />
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
    </Panel>
  );
}

/// The cells of a list, each chosen by its id, with its title.
function CellRows({ flow, ids, onChoose }: { flow: Flow; ids: string[]; onChoose: (c: Chosen) => void }) {
  return (
    <ul className="uses">
      {ids.map((id) => (
        <ListRow key={id}>
          <Choose chosen={{ cell: id }} onChoose={onChoose}>
            <span className="mono">{id}</span>
          </Choose>
          <span>{flow.cells.find((c) => c.id === id)?.title}</span>
        </ListRow>
      ))}
    </ul>
  );
}

/// Everything about a line: the file it is written in, its keys as that file writes them, and the cells on it, which
/// share its session, in the order of the walk.
export function LinePanel(props: { row: LineRow; flow: Flow; onChoose: (c: Chosen) => void }) {
  const { row, flow, onChoose } = props;
  const cells = flow.cells.filter((c) => c.kind === 'agent' && c.line === row.name).map((c) => c.id);
  return (
    <Panel
      label={`Line ${row.name}`}
      title={<span className="mono line-name">{row.name}</span>}
      belongs={row.line && <span className="mono">{row.line.file}</span>}
      lines={(row.line ? lineKeys(row.line) : []).map(([key, value]) => (
        <StatusLine key={key} word={<Attr name={key} value={value} />} />
      ))}
    >
      <PanelPart label="cells" count={cells.length}>
        <CellRows flow={flow} ids={cells} onChoose={onChoose} />
      </PanelPart>
    </Panel>
  );
}

/// Everything about a named test: the file it is written in, its keys as that file writes them, each block it reads
/// chosen by its name, and the cells that name it, by the key that names it: those that run only when it holds, and
/// those the work waits before until it holds.
export function TestPanel(props: { row: ConditionRow; flow: Flow; onChoose: (c: Chosen) => void }) {
  const { row, flow, onChoose } = props;
  const test = row.test;
  if (!test) return null;
  const names = (keys: Condition[]) => keys.some((k) => k.on === 'test' && k.test.name === test.name);
  const when = flow.cells.filter((c) => names(c.when)).map((c) => c.id);
  const requires = flow.cells.filter((c) => names(c.requires)).map((c) => c.id);
  return (
    <Panel
      label={`Test ${test.name}`}
      title={<span className="mono">{test.name}</span>}
      belongs={<span className="mono">{test.file}</span>}
      lines={
        <>
          <StatusLine word={<span className="mono">says</span>}>{test.words}</StatusLine>
          <StatusLine word={<Attr name="kind" value={snake(test.kind)} />} />
          <StatusLine
            word={
              <Attr
                name="reads"
                value={
                  <>
                    [
                    {test.reads.map((block, i) => (
                      <Fragment key={block}>
                        {i > 0 && ', '}
                        <Choose chosen={{ block }} onChoose={onChoose}>
                          {block}
                        </Choose>
                      </Fragment>
                    ))}
                    ]
                  </>
                }
              />
            }
          />
          {test.is !== null && <StatusLine word={<Attr name="is" value={test.is} />} />}
        </>
      }
    >
      {when.length > 0 && (
        <PanelPart label="when" count={when.length}>
          <CellRows flow={flow} ids={when} onChoose={onChoose} />
        </PanelPart>
      )}
      {requires.length > 0 && (
        <PanelPart label="requires" count={requires.length}>
          <CellRows flow={flow} ids={requires} onChoose={onChoose} />
        </PanelPart>
      )}
    </Panel>
  );
}

/// One of the engine's tools, which every agent's cell of the flow may ask for: the file it is written in, and each of
/// its keys with its text as that file writes it.
export function ToolPanel(props: {
  name: string;
  /// The tool's file; none where the page was not given it.
  words?: ActWords;
  /// Why what an agent is sent, the tools' files with it, could not be read.
  unread?: string;
}) {
  const { name, words, unread } = props;
  return (
    <Panel
      label={`Tool ${name}`}
      title={<span className="mono">{name}</span>}
      belongs={words && <span className="mono">{words.source}</span>}
    >
      {words ? (
        <ul className="uses">
          {words.words.map(([key, pieces]) => (
            <ListRow key={key}>
              <span className="mono">{key}</span>
              <div className="row-note">
                <SentText pieces={pieces} />
              </div>
            </ListRow>
          ))}
        </ul>
      ) : unread !== undefined ? (
        <p className="read-failed" role="alert">
          The tool's file could not be read: {unread}
        </p>
      ) : (
        <p className="muted">The page was not given the tool's file</p>
      )}
    </Panel>
  );
}

/// Everything about a group of the flow's blocks: the file that names it, its blocks as that file names them, each
/// chosen by its name, the cells whose file watches it and those whose file reads it, and the ways back through it.
export function GroupPanel(props: { name: string; flow: Flow; onChoose: (c: Chosen) => void }) {
  const { name, flow, onChoose } = props;
  const blocks = flow.groups.find((g) => g.name === name)?.blocks ?? [];
  const named = (list: { name: string }[]) => list.some((n) => n.name === name);
  const runs = flow.cells.filter((c) => named(c.watches));
  const reads = flow.cells.filter((c) => named(c.reads));
  // Each cell that watches it, with the cells after it that write a block of the group and whose write can run it.
  const at = (id: string) => flow.cells.findIndex((c) => c.id === id);
  const backs = runs.flatMap((reader) => {
    const writers = flow.cells
      .filter((w) => at(w.id) > at(reader.id) && blocks.some((b) => w.outputs.includes(b)) && canRun(w, reader))
      .map((w) => w.id);
    return writers.length > 0 ? [{ reader: reader.id, writers }] : [];
  });
  return (
    <Panel label={`Group ${name}`} title={<span className="mono">{name}</span>} belongs={<span className="mono">flow.toml</span>}>
      <PanelPart label="blocks" count={blocks.length}>
        <ul className="uses">
          {blocks.map((block) => (
            <ListRow key={block}>
              <Choose chosen={{ block }} onChoose={onChoose}>
                <span className="mono">{block}</span>
              </Choose>
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      {runs.length > 0 && (
        <PanelPart label="watches" count={runs.length}>
          <CellRows flow={flow} ids={runs.map((c) => c.id)} onChoose={onChoose} />
        </PanelPart>
      )}
      {reads.length > 0 && (
        <PanelPart label="reads" count={reads.length}>
          <CellRows flow={flow} ids={reads.map((c) => c.id)} onChoose={onChoose} />
        </PanelPart>
      )}
      {backs.length > 0 && (
        <PanelPart label="way_back" count={backs.length}>
          <ul className="uses">
            {backs.map(({ reader, writers }) => (
              <ListRow key={reader}>
                <span className="mono">
                  {writers.join(', ')} <Arrow to="right" />
                  <span className="visually-hidden">to</span> {reader}
                </span>
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
    </Panel>
  );
}

/// Everything about what every agent's cell reads beside what its own file names, as `flow.toml` writes it
/// (`[every_agent]`): each name as written, with the data it stands for where it stands for others than itself, each
/// chosen by its name, and its kind.
export function EveryAgentPanel(props: { flow: Flow; onChoose: (c: Chosen) => void }) {
  const { flow, onChoose } = props;
  const every = flow.everyAgent;
  const list = (names: { name: string; blocks: string[] }[]) =>
    names.length > 0 && (
      <PanelPart label="reads" count={names.length}>
        <ul className="uses">
          {names.map((n) => {
            const itself = namesItself(n);
            return (
              <ListRow key={n.name}>
                {itself ? (
                  <Choose chosen={{ block: n.name }} onChoose={onChoose}>
                    <span className="mono">{n.name}</span>
                  </Choose>
                ) : (
                  <span className="mono">{n.name}</span>
                )}
                {!itself && (
                  <Attr
                    name="data"
                    value={
                      <>
                        [
                        {n.blocks.map((block, i) => (
                          <Fragment key={block}>
                            {i > 0 && ', '}
                            <Choose chosen={{ block }} onChoose={onChoose}>
                              {block}
                            </Choose>
                          </Fragment>
                        ))}
                        ]
                      </>
                    }
                  />
                )}
                {every.kinds[n.name] && <Attr name="kind" value={inputKindWords(every.kinds[n.name])} />}
              </ListRow>
            );
          })}
        </ul>
      </PanelPart>
    );
  return (
    <Panel label="every_agent" title={<span className="mono">every_agent</span>} belongs={<span className="mono">flow.toml</span>}>
      {list(every.reads)}
    </Panel>
  );
}
