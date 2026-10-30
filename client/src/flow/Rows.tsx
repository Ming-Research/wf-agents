// A cell's rows, as the two tables both panels show (Sides.tsx). From: every row of the flow that leads into the cell,
// by the cell it is of, its name and its condition, a move of many cells once with all of them, and the engine's
// fallback, which brings the cell back to itself, as one line. To: every row of the cell's own transition table, by
// its name, its condition, the cell it leads to and what it does, its own answers first, the main path first among
// them, then its moves, and the engine's fallback, the same for every agent cell, folded at the end. A condition names
// what of the return it is worked out from as the return holds it ("`outcome.take.way` is `do_now`"), or the engine's
// value it reads. On a round, the row it came in by and the row it left by are marked as the row picked, with the
// round on the other side beside each, and every other row is greyed.

import { ChevronDownIcon, ChevronRightIcon } from '@primer/octicons-react';
import { Label } from '@primer/react';
import { Fragment, type ReactNode, useState } from 'react';

import type { CompiledCell, CompiledGraph, CompiledRow, RoundSummary, RowRef } from '../api';
import { actionWords, fallback, mainPath, type Move, moves, rowActions } from '../graph/flowModel';
import { LinkButton } from '../parts';
import { t, tn } from '../text';
import { joined, Name } from '../voices';
import { type Flow, useFlow } from './context';
import { CellCode, CellLink, conditionsWords, inline, placeIn, Target } from './words';

/// Where a cell is entered from, by kind: other cells' own answers that lead here, and the cell's own back to itself;
/// the rows back here after a move (from the tools, a pause, answers, an ask, or to where the work stopped); the
/// moves of other cells that lead here, each with the cells that make it; and whether the engine's fallback brings the
/// cell back to itself.
export type EnteredFrom = {
  answers: RowRef[];
  back: RowRef[];
  moves: Array<{ row: string; cells: string[] }>;
  fallback: boolean;
};

export function enteredFrom(g: CompiledGraph, refs: RowRef[], id: string): EnteredFrom {
  const out: EnteredFrom = { answers: [], back: [], moves: [], fallback: false };
  for (const ref of refs) {
    const row = g.cells.find((x) => x.id === ref.cell)?.rows[ref.row];
    if (row === undefined) continue;
    if (fallback(row)) {
      if (ref.cell === id) out.fallback = true;
    } else if (row.common) {
      if (ref.cell === id) continue;
      const m = out.moves.find((x) => x.row === row.name);
      if (m === undefined) out.moves.push({ row: row.name, cells: [ref.cell] });
      else if (!m.cells.includes(ref.cell)) m.cells.push(ref.cell);
    } else if (row.to.startsWith('@') && ref.cell !== id) {
      out.back.push(ref);
    } else {
      out.answers.push(ref);
    }
  }
  return out;
}

/// Where a cell leaves to, by kind, each a list of its rows by their places: its own answers, those on the main path
/// first; the moves it makes of its own; and the engine's fallback.
export function leavesTo(c: CompiledCell): { answers: number[]; moves: number[]; fallback: number[] } {
  const all = c.rows.map((_, i) => i);
  const answers = all.filter((i) => !c.rows[i].common && !fallback(c.rows[i]));
  return {
    answers: [...answers.filter((i) => c.rows[i].main), ...answers.filter((i) => !c.rows[i].main)],
    moves: all.filter((i) => c.rows[i].common && !fallback(c.rows[i])),
    fallback: all.filter((i) => fallback(c.rows[i])),
  };
}

/// What the engine does for a row of its fallback, as the flow's section on rounds says it; none in a flow version
/// stored before it said it so.
export function fallbackWords(g: CompiledGraph, r: CompiledRow): string | null {
  const rounds = g.rounds;
  const a = r.if[0];
  if (rounds == null || a === undefined) return null;
  if (a[0] === 'sent_back') return a.length === 2 ? rounds.sentBack.pause : rounds.sentBack.desc;
  if (a[0] === 'failed') return rounds.failed.kinds.find((k) => k.name === a[1])?.desc ?? null;
  if (a[0] === 'engine') return rounds.engine?.find((k) => k.name === a[1])?.desc ?? null;
  return null;
}

/// The moves the legend gives most cells that could make them, which this cell does not make: said under its rows.
export function lacksMoves(g: CompiledGraph, c: CompiledCell): Move[] {
  return moves(g).filter((m) => m.tier === 'marked' && m.could.includes(c.id) && !m.cells.includes(c.id));
}

/// The row a round left its cell by: the one recorded; for a return sent back, the fallback's row for it, the one for
/// sending back too often once that many of the cell's returns in a row were; for a round cut off, the fallback's row
/// for that; none while it runs, or for a run that failed, whose kind the round does not record.
export function leftBy(c: CompiledCell | undefined, r: RoundSummary, rounds: RoundSummary[]): string | null {
  if (r.transition !== null) return r.transition;
  if (c === undefined) return null;
  if (r.outcome === 'sent_back') {
    let streak = 0;
    for (const x of [...rounds].reverse()) {
      if (x.n > r.n) continue;
      if (x.cell !== r.cell || x.outcome !== 'sent_back') break;
      streak += 1;
    }
    const row = c.rows.find((x) => {
      const a = x.if[0];
      if (!fallback(x) || a === undefined || a[0] !== 'sent_back') return false;
      const n = (a as [string, number?])[1];
      return n === undefined || n <= Math.max(1, streak);
    });
    return row?.name ?? null;
  }
  if (r.outcome === 'interrupted') {
    return c.rows.find((x) => fallback(x) && x.if[0]?.[0] === 'engine' && x.if[0][1] === 'cut_off')?.name ?? null;
  }
  return null;
}

/// What a round took, for the tables to mark: the row it came in by, as the cell it came from and the row's name, and
/// the row it left by, by its name; none where it came in by no row (it started the work) or has left by none yet.
/// `came` and `went` say so over a table none of whose rows is the one; `before` and `after` write the round on the
/// other side, with its cell where the row does not name one.
export type Taken = {
  from: { cell: string; row: string } | null;
  to: string | null;
  came: ReactNode;
  went: ReactNode;
  before: (withCell: boolean) => ReactNode;
  after: (withCell: boolean) => ReactNode;
};

/// A row's condition in words, the places of the return named as its cell's return holds them.
function when(f: Flow, c: CompiledCell, r: CompiledRow): string {
  return conditionsWords(r.if, new Set(Object.keys(f.compiled.engine.values)), placeIn(c, f.g.within));
}

/// One line of From: the cells whose row it is, the row's name (words, for the fallback), its condition, what the flow
/// notes of it, and the names of the rows it stands for.
type Into = { cells: string[]; row: string; group: boolean; when: string; note: string | null; rows: string[] };

/// Every row that leads into a cell, a line each: the rows of other cells' own answers and of the cell's own back to
/// itself, the ways back after a move, each move of other cells once with every cell that makes it, and the engine's
/// fallback, which brings the cell back to itself.
export function intoRows(f: Flow, c: CompiledCell): Into[] {
  const into = enteredFrom(f.g, f.cellView(c.id)?.entered_from ?? [], c.id);
  const out: Into[] = [];
  for (const ref of [...into.answers, ...into.back]) {
    const src = f.cell(ref.cell);
    const row = src?.rows[ref.row];
    if (src === undefined || row === undefined) continue;
    out.push({ cells: [ref.cell], row: row.name, group: false, when: when(f, src, row), note: row.note ?? null, rows: [row.name] });
  }
  for (const m of into.moves) {
    const src = f.cell(m.cells[0]);
    const row = src?.rows.find((x) => x.name === m.row);
    out.push({
      cells: m.cells,
      row: m.row,
      group: false,
      when: src !== undefined && row !== undefined ? when(f, src, row) : '',
      note: row?.note ?? null,
      rows: [m.row],
    });
  }
  if (into.fallback) {
    const rows = c.rows.filter(fallback).map((x) => x.name);
    out.push({ cells: [c.id], row: t('from_fallback_row'), group: true, when: t('from_fallback_when'), note: rows.join(', '), rows });
  }
  return out;
}

/// A row's name as a code, broken only after an underscore where its column is narrow.
function RowName({ name }: { name: string }) {
  return (
    <Name>
      {name.split('_').map((part, i) =>
        i === 0 ? (
          part
        ) : (
          <Fragment key={i}>
            _<wbr />
            {part}
          </Fragment>
        ),
      )}
    </Name>
  );
}

/// The class of a line of a table on a round: the one taken, or another, greyed; none on a cell's panel.
const lineClass = (taken: Taken | undefined, hit: boolean) => (taken === undefined ? undefined : hit ? 'side-taken' : 'side-other');

/// The cells a line of From comes from: the cell by its code and title, itself, or many by their codes.
function Sources({ cells, here }: { cells: string[]; here: string }) {
  if (cells.length === 1) return cells[0] === here ? <>{t('entered_itself')}</> : <CellLink id={cells[0]} />;
  return (
    <>
      {cells.map((x, i) => (
        <Fragment key={x}>
          {i > 0 ? ', ' : null}
          <CellCode id={x} />
        </Fragment>
      ))}
    </>
  );
}

/// The table of the rows that lead into a cell; on a round, the one it came in by marked, the round it came from beside.
export function FromTable({ c, taken }: { c: CompiledCell; taken?: Taken }) {
  const f = useFlow();
  const rows = intoRows(f, c);
  const from = taken?.from ?? null;
  const hit = from !== null ? rows.findIndex((x) => x.cells.includes(from.cell) && x.rows.includes(from.row)) : -1;
  return (
    <>
      {taken !== undefined && hit < 0 && taken.came !== null ? <p className="pp-plain">{taken.came}</p> : null}
      {rows.length === 0 ? (
        <p className="pp-note">{t('entered_none')}</p>
      ) : (
        <table className="list side-table">
          <thead>
            <tr>
              <th>{t('col_from')}</th>
              <th>{t('col_row')}</th>
              <th>{t('col_when')}</th>
              {taken !== undefined ? <th>{t('col_round')}</th> : null}
            </tr>
          </thead>
          <tbody>
            {rows.map((x, i) => (
              <tr key={`${x.cells.join(',')}:${x.row}`} className={lineClass(taken, i === hit)}>
                <td>
                  <Sources cells={x.cells} here={c.id} />
                </td>
                <td title={x.note ?? undefined}>{x.group ? x.row : <RowName name={x.row} />}</td>
                <td>{inline(x.when)}</td>
                {taken !== undefined ? <td>{i === hit ? taken.before(x.cells.length > 1) : null}</td> : null}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  );
}

/// What a row does besides leading on, in small words under where it leads: its actions, the values it records, the
/// counter it counts, and whether it waits, pauses the work or takes an interjection; what each is, on hovering.
function Does({ r }: { r: CompiledRow }) {
  const f = useFlow();
  const sets = Object.entries(r.sets ?? {});
  const counted = r.counts ? (f.g.counters.find((k) => k.name === r.counts)?.desc ?? undefined) : undefined;
  const words = joined([
    ...rowActions(r).map((a) => (
      <span key={a} title={f.compiled.engine.actions[a] ?? undefined}>
        {actionWords(a)}
      </span>
    )),
    sets.length > 0 ? <span key="sets">{inline(t('row_records_words', { what: sets.map(([k, v]) => `\`${k} = ${v}\``).join(', ') }))}</span> : null,
    r.counts ? (
      <span key="counts" title={counted}>
        {inline(t('row_counts_words', { counter: `\`${r.counts}\`` }))}
      </span>
    ) : null,
    r.waits ? t('row_waits') : null,
    r.pauses ? t('row_pauses') : null,
    r.interject ? t('row_interjection') : null,
  ]);
  return words.length > 0 ? <span className="side-does">{words}</span> : null;
}

/// The table of a cell's own rows; on a round, the one it left by marked, the round it led to beside.
export function ToTable({ c, taken }: { c: CompiledCell; taken?: Taken }) {
  const f = useFlow();
  const [shown, setShown] = useState(false);
  const out = leavesTo(c);
  const hit = taken?.to != null ? c.rows.findIndex((r) => r.name === taken.to) : -1;
  // The fallback is open where the round left by one of its rows.
  const open = shown || out.fallback.includes(hit);
  const cols = taken !== undefined ? 4 : 3;
  const line = (i: number) => {
    const r = c.rows[i];
    return (
      <tr key={i} className={lineClass(taken, i === hit)}>
        <td title={r.note ?? fallbackWords(f.g, r) ?? undefined}>
          <RowName name={r.name} />
          {mainPath(r) ? (
            <>
              {' '}
              <Label variant="secondary">{t('row_main')}</Label>
            </>
          ) : null}
        </td>
        <td>{inline(when(f, c, r))}</td>
        <td>
          <Target c={c} row={i} />
          <Does r={r} />
        </td>
        {taken !== undefined ? <td>{i === hit ? taken.after(r.to.startsWith('@')) : null}</td> : null}
      </tr>
    );
  };
  const lacks = lacksMoves(f.g, c);
  const does = c.actions ?? [];
  return (
    <>
      {taken !== undefined && hit < 0 && taken.went !== null ? <p className="pp-plain">{taken.went}</p> : null}
      {c.rows.length === 0 ? (
        <p className="pp-note">{t('leaves_none')}</p>
      ) : (
        <table className="list side-table">
          <thead>
            <tr>
              <th>{t('col_row')}</th>
              <th>{t('col_when')}</th>
              <th>{t('col_to')}</th>
              {taken !== undefined ? <th>{t('col_round')}</th> : null}
            </tr>
          </thead>
          <tbody>
            {[...out.answers, ...out.moves].map(line)}
            {out.fallback.length > 0 ? (
              <tr className="side-fold">
                <td colSpan={cols}>
                  <LinkButton onClick={() => setShown(!open)}>
                    {open ? <ChevronDownIcon /> : <ChevronRightIcon />} {tn('to_fallback', out.fallback.length)}
                  </LinkButton>
                </td>
              </tr>
            ) : null}
            {open ? out.fallback.map(line) : null}
          </tbody>
        </table>
      )}
      {lacks.map((m) => (
        <p key={m.row} className="pp-note">
          {t(m.engine ? 'leaves_lacks_engine' : 'chart_lacks_title', { cell: c.id, move: actionWords(m.row), to: m.to })}
        </p>
      ))}
      {does.length > 0 ? (
        <p className="pp-note">
          {t('leaves_does')}{' '}
          {joined(
            does.map((a) => (
              <span key={a} title={f.compiled.engine.actions[a] ?? undefined}>
                {actionWords(a)}
              </span>
            )),
          )}
        </p>
      ) : null}
    </>
  );
}
