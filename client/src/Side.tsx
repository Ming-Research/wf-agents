// The panel docked at the chart's right: everything about the cell or block the owner chose.

import { Fragment, type ReactNode, useState } from 'react';

import type { ActWords, Block, Cell, Fault, Flow, SentCarried, SentPart, SentPiece, Unknown } from './api';
import type { Chosen } from './Chart';
import { FormTree } from './FormTree';
import { type BlockRow, type Matrix, markOf, where } from './matrix';
import { Attr, ListRow, Panel, PanelPart, StatusLine } from './parts';
import {
  charactersWords,
  conditionWords,
  faultWords,
  inputKindWords,
  listed,
  readsWords,
  runBy,
  testWords,
  thereWhen,
  writersBesideCells,
} from './words';

/// Whether the panel is folded away, and the way to fold it or bring it back.
export type Fold = { folded: boolean; onFold: () => void; edge?: ReactNode };

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
/// the program's own with the form, which the program makes a tool of; the system prompt; the message. Each part shows its key in the files, its attributes, the file its words
/// come from, and its text; a text every cell of the line is sent alike, or a long one, is folded.
function AgentSent({
  parts,
  line,
  cells,
  unknown,
  tools,
  onChoose,
}: {
  parts: SentPart[];
  line: string;
  cells: string[];
  unknown: Unknown[];
  /// The names of the engine's tools, which the form's tree draws apart from the fields.
  tools: string[];
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
        {part.kind && <Attr name="kind" value={part.kind} />}
        <span className="muted mono">{part.source}</span>
        {part.pieces.length === 0 ? null : fold ? (
          <>
            <span className="muted">{charactersWords(sizeOf(part))}</span>
            <Folded label="the text">
              {part.what === 'form' ? (
                <FormTree text={textOf(part.pieces)} tools={tools} />
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
/// what it is given and who writes each, what it may write and who reads each, the tools it may ask the engine for,
/// what its agent is sent as it is entered, when it runs, what it waits for, its limits and the cells it shares a
/// session with.
export function CellPanel(props: {
  cell: Cell;
  matrix: Matrix;
  flow: Flow;
  /// What the cell's agent is sent as the cell is entered, part by part in the order sent; none for an engine's
  /// cell, and where the page was not given it.
  sent?: SentPart[];
  /// The names of the engine's tools; none where the page was not given them.
  tools?: string[];
  /// What an engine's cell's act writes; none for an agent's cell, and where the page was not given it.
  act?: ActWords;
  /// The names a text sent to an agent gives that nothing defines.
  unknown?: Unknown[];
  onChoose: (c: Chosen) => void;
  fold: Fold;
}) {
  const { cell, flow, sent, tools, act, unknown, onChoose, fold } = props;
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const onLine = cell.line ? (flow.lines.find((l) => l.name === cell.line)?.cells ?? []) : [];
  const sharing = onLine.filter((c) => c !== cell.id);
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
          {cell.condition && (
            <StatusLine word={<Attr name="when" value={conditionWords(cell.condition)} />}>
              {cell.condition.on === 'test' ? cell.condition.test.words : undefined}
            </StatusLine>
          )}
          {cell.preconditions.length > 0 && (
            <StatusLine word={<Attr name="requires" value={listed(cell.preconditions.map(testWords))} />} />
          )}
          {cell.takesMain && <StatusLine word={<Attr name="takes_main" value="true" />} />}
          <OwnLimits cell={cell} flow={flow} />
        </>
      }
      {...fold}
    >
      {sent && sent.length > 0 ? (
        <>
          <PanelPart label="triggered_by" count={cell.triggers.length}>
            <ul className="uses">
              {cell.triggers.map((t) => (
                <ListRow key={t.name}>
                  <span className="mono">{t.name}</span>
                  <Attr
                    name="block"
                    value={
                      <Choose chosen={{ block: t.block }} onChoose={onChoose}>
                        {t.block}
                      </Choose>
                    }
                  />
                  {t.read.length > 0 && <Attr name="read" value={listed(t.read)} />}
                  {t.do && (
                    <div className="row-note">
                      <Attr name="do" value="" />
                      {t.do}
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
            tools={tools ?? []}
            onChoose={onChoose}
          />
        </>
      ) : (
        <>
          <PanelPart label="triggered_by" count={cell.runsOn.length}>
            <ul className="uses">
              {cell.runsOn.map((name, place) => {
                const block = blocks.get(name) as Block;
                const back = later(block.writtenBy.cells.filter((id) => id !== cell.id));
                return (
                  <ListRow key={name}>
                    <span
                      className="muted mono"
                      title="Its place among what runs the cell, in the order the cell names them"
                    >
                      {place + 1}
                    </span>
                    <Choose chosen={{ block: name }} onChoose={onChoose}>
                      <span className="mono">{name}</span>
                    </Choose>
                    <WrittenBy flow={flow} cell={cell} block={block} onChoose={onChoose} />
                    {back.length > 0 && (
                      <span className="way-back">
                        <Attr name="way_back" value={listed(back)} />
                      </span>
                    )}
                    {block.what !== null && <span className="row-note">{block.what}</span>}
                    {cell.input[name]?.runs?.why && (
                      <span className="row-note">
                        <Attr name="why" value={cell.input[name].runs?.why} />
                      </span>
                    )}
                  </ListRow>
                );
              })}
            </ul>
          </PanelPart>

          {cell.kind === 'agent' && (
            <PanelPart label="context" count={cell.given.length + 1}>
              <ul className="uses">
                <ListRow key="task">
                  <span
                    className="muted mono"
                    title="Its place among what the cell is given, in the order it is given them"
                  >
                    1
                  </span>
                  <span className="mono">task</span>
                </ListRow>
                {cell.given.map((name, place) => {
                  const block = blocks.get(name) as Block;
                  const given = cell.input[name]?.given ?? null;
                  return (
                    <ListRow key={name}>
                      <span
                        className="muted mono"
                        title="Its place among what the cell is given, in the order it is given them"
                      >
                        {place + 2}
                      </span>
                      <Choose chosen={{ block: name }} onChoose={onChoose}>
                        <span className="mono">{name}</span>
                      </Choose>
                      <Attr name="kind" value={inputKindWords(given?.kind ?? null)} />
                      <WrittenBy flow={flow} cell={cell} block={block} onChoose={onChoose} />
                      {block.what !== null && <span className="row-note">{block.what}</span>}
                      {given?.why && (
                        <span className="row-note">
                          <Attr name="why" value={given.why} />
                        </span>
                      )}
                      {thereWhen(flow, cell.id, block) !== '' && (
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
                const above = block.readBy.cells.filter((id) => where(flow, cell.id, id) === 'above');
                const aboveConditions = block.readBy.conditions.filter((id) => where(flow, cell.id, id) === 'above');
                const sendsBack = [...new Set([...above, ...aboveConditions])];
                return (
                  <ListRow key={name}>
                    <Choose chosen={{ block: name }} onChoose={onChoose}>
                      <span className="mono">{name}</span>
                    </Choose>
                    {cell.mustWrite.includes(name) && <Attr name="must_write" value="true" />}
                    {cell.given.includes(name) && <Attr name="given" value="true" />}
                    <span>
                      <Readers flow={flow} from={cell.id} block={block} onChoose={onChoose} />
                    </span>
                    {sendsBack.length > 0 && (
                      <span className="way-back">
                        <Attr name="way_back" value={listed(sendsBack)} />
                      </span>
                    )}
                    {block.what !== null && <span className="row-note">{block.what}</span>}
                  </ListRow>
                );
              })}
            </ul>
          </PanelPart>

          {cell.kind === 'agent' && (
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

/// Everything about a block: its values, who writes it, who reads it, and the ways back through it.
export function BlockPanel(props: {
  row: BlockRow;
  matrix: Matrix;
  flow: Flow;
  onChoose: (c: Chosen) => void;
  fold: Fold;
}) {
  const { row, flow, onChoose, fold } = props;
  const block = row.block;
  const besides = writersBesideCells(block);
  const backs = flow.cells.flatMap((reader, r) => {
    const mark = markOf(reader, block.name);
    if (!mark.runs && !mark.condition) return [];
    const writers = flow.cells.filter((w, i) => i > r && markOf(w, block.name).write).map((w) => w.id);
    return writers.length > 0 ? [{ reader: reader.id, writers, byCondition: !mark.runs }] : [];
  });
  return (
    <Panel
      label={`Block ${block.name}`}
      title={<span className="mono">{block.name}</span>}
      belongs={block.values ? <Attr name="values" value={listed(block.values)} /> : undefined}
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
      {...fold}
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
            {backs.map(({ reader, writers, byCondition }) => (
              <ListRow key={reader}>
                <span className="way-back mono">
                  {writers.join(', ')} → {reader}
                </span>
                <span className="muted mono">{byCondition ? 'when' : 'triggered_by'}</span>
              </ListRow>
            ))}
          </ul>
        </PanelPart>
      )}
    </Panel>
  );
}
