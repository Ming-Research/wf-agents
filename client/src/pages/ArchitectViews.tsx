// What an architect hands back, each drawn by its shape in the flow's formats: the policy, the study of a new one, its
// probes and the readings of it by a new architect; an item change its batch made; its judgments of the whole and each
// matter it took with what each round of its path answered, in its log; its understanding of the project with its
// versions, and the check of the policy against the constitution. A value not of the shape expected is shown as
// labelled text, never as a tree of JSON.

import { type ReactNode, useState } from 'react';

import type { JsonValue, LogEntry, Said, Understanding } from '../api';
import { againstInOrder, type LineChange, stateChanges } from '../architect';
import { time } from '../format';
import { Field, Fold } from '../item/Shapes';
import { empty, isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { t } from '../text';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);
const texts = (v: JsonValue | undefined): string[] => (Array.isArray(v) ? v.map((x) => str(x)).filter((x) => x !== '') : []);

/// A list of texts, when there are any.
function Texts({ label, value, ordered = false }: { label: string; value: JsonValue | undefined; ordered?: boolean }) {
  const list = texts(value);
  if (list.length === 0) return null;
  const items = list.map((x, i) => (
    <li key={i}>
      <MarkdownInline text={x} />
    </li>
  ));
  return (
    <div className="field">
      <span className="decision-label">{label}</span>
      {ordered ? <ol className="arch-list">{items}</ol> : <ul className="arch-list">{items}</ul>}
    </div>
  );
}

/// A round of a wake, by its name, that opens it on the chart.
export type OpenRound = (work: number, round: number) => void;

export function RoundLink({ work, round, onRound }: { work: number; round: number; onRound: OpenRound }) {
  return (
    <button type="button" className="link" onClick={() => onRound(work, round)}>
      R{work}-{round}
    </button>
  );
}

/// When a round handed it back, and the round.
function From({ said, onRound, label }: { said: Said; onRound: OpenRound; label?: string }) {
  return (
    <span className="muted arch-from">
      {label !== undefined ? `${label} ` : ''}
      {said.at !== null ? `${time(said.at)} · ` : ''}
      <RoundLink work={said.work} round={said.round} onRound={onRound} />
    </span>
  );
}

/// A policy: its goal and why, its points numbered, its stages in order, when to report and what it governs.
export function PolicyText({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.goal !== 'string') return <Labelled value={value} />;
  const points = objs(value.points);
  const signals = objs(value.signals);
  return (
    <div className="shape">
      <Field label={t('policy_goal')} value={value.goal} />
      <Field label={t('policy_why')} value={value.why} />
      {points.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('policy_points')}</span>
          <ol className="arch-list arch-numbered">
            {points.map((p, i) => (
              <li key={i} value={typeof p.no === 'number' ? p.no : i + 1}>
                <Markdown text={str(p.text)} />
              </li>
            ))}
          </ol>
        </div>
      ) : null}
      <Texts label={t('policy_stages')} value={value.stages} ordered />
      {signals.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('policy_signals')}</span>
          <ul className="arch-list">
            {signals.map((s, i) => (
              <li key={i}>
                <strong>
                  <MarkdownInline text={str(s.what)} />
                </strong>
                : <MarkdownInline text={str(s.how)} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <Field label={t('policy_scope')} value={value.scope} />
    </div>
  );
}

/// The study of a new policy: what the owner wants, how things stand, what it touches, the ways compared and the one
/// recommended, what its conclusions rest on, and what it asks the owner.
export function StudyView({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.intent !== 'string') return <Labelled value={value} />;
  const impact = objs(value.impact);
  const options = objs(value.options);
  const basis = objs(value.basis);
  return (
    <div className="shape">
      <Field label={t('study_intent')} value={value.intent} />
      <Field label={t('study_now')} value={value.now} />
      {impact.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('study_impact')}</span>
          <ul className="arch-list">
            {impact.map((x, i) => (
              <li key={i}>
                <strong>
                  <MarkdownInline text={str(x.what)} />
                </strong>
                : <MarkdownInline text={str(x.how)} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {options.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('study_options')}</span>
          <div className="option-boxes">
            {options.map((o, i) => (
              <div key={i} className="option-box">
                <Markdown text={str(o.does)} />
                <Field label={t('study_then')} value={o.then} />
              </div>
            ))}
          </div>
        </div>
      ) : null}
      <Field label={t('study_recommend')} value={value.recommend} />
      {basis.length > 0 ? (
        <Fold title={t('study_basis', { n: basis.length })}>
          <ul className="grounds">
            {basis.map((b, i) => (
              <li key={i}>
                <span className={`badge ${str(b.kind) === 'checked' ? 'ground-verified' : 'ground-inferred'}`}>
                  {words(str(b.kind))}
                </span>{' '}
                <MarkdownInline text={str(b.claim)} />
                <Field label={t('study_where')} value={b.where} />
              </li>
            ))}
          </ul>
        </Fold>
      ) : null}
      <Texts label={t('study_questions')} value={value.questions} />
    </div>
  );
}

/// The probes of a policy being set: each question, the points it probes and the answer the architect holds right.
export function ProbesView({ value }: { value: JsonValue }) {
  const probes = objs(value);
  if (probes.length === 0) return null;
  return (
    <ul className="arch-list">
      {probes.map((p, i) => (
        <li key={i}>
          <strong>{str(p.id)}</strong>
          {Array.isArray(p.points) && p.points.length > 0 ? (
            <span className="muted"> · {t('probe_points', { points: p.points.map((x) => str(x)).join(', ') })}</span>
          ) : null}
          <Markdown text={str(p.ask)} />
          <Field label={t('probe_expect')} value={p.expect} />
        </li>
      ))}
    </ul>
  );
}

/// The architect's verdict on a new architect's reading: understood, or the gaps it found.
function ReadingVerdict({ value }: { value: JsonValue }) {
  if (!isObj(value)) return <span className="badge">{t('reading_not_judged')}</span>;
  const gaps = objs(value.gaps);
  return (
    <>
      <span className={`badge ${value.verdict === 'understood' ? 'verdict-good' : 'verdict-bad'}`}>
        {words(str(value.verdict))}
      </span>
      <Field label={t('reading_why')} value={value.why} />
      {gaps.length > 0 ? (
        <ul className="arch-list">
          {gaps.map((g, i) => (
            <li key={i}>
              <strong>
                <MarkdownInline text={str(g.what)} />
              </strong>
              <Field label={t('reading_seen')} value={g.seen} />
              <Field label={t('reading_missing')} value={g.missing} />
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}

/// A new architect's reading of the policy, folded under the verdict on it: its restatement point by point, what it
/// asked, and its answers to the probes.
export function ReadingView({ round, reading, verdict }: { round: string; reading: JsonValue; verdict: JsonValue }) {
  const r = isObj(reading) ? reading : {};
  const restate = objs(r.restate);
  const answers = objs(r.answers);
  return (
    <div className="arch-reading">
      <div className="work-item-head">
        <strong>{t('reading_head', { round })}</strong>
      </div>
      <ReadingVerdict value={verdict} />
      <Fold title={t('reading_text')}>
        {restate.length > 0 ? (
          <ol className="arch-list arch-numbered">
            {restate.map((p, i) => (
              <li key={i} value={typeof p.no === 'number' ? p.no : i + 1}>
                <Markdown text={str(p.text)} />
              </li>
            ))}
          </ol>
        ) : null}
        <Texts label={t('reading_asks')} value={r.asks} />
        {answers.length > 0 ? (
          <div className="field">
            <span className="decision-label">{t('reading_answers')}</span>
            <ul className="arch-list">
              {answers.map((a, i) => (
                <li key={i}>
                  <strong>{str(a.id)}</strong>: <MarkdownInline text={str(a.answer)} />
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </Fold>
    </div>
  );
}

/// A field's value on one line where it is short: a text as it reads, a list of texts or numbers as a list, anything
/// else as labelled text.
function FactValue({ value }: { value: JsonValue }) {
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return <MarkdownInline text={String(value)} />;
  }
  if (Array.isArray(value) && value.every((x) => typeof x === 'string' || typeof x === 'number')) {
    return value.length === 1 ? (
      <MarkdownInline text={String(value[0])} />
    ) : (
      <ul className="arch-list">
        {value.map((x, i) => (
          <li key={i}>
            <MarkdownInline text={String(x)} />
          </li>
        ))}
      </ul>
    );
  }
  return <Labelled value={value} />;
}

/// One change to the database the architect collected: its kind and, for what it creates, the title; why; and what
/// it does, each field beside its name.
export function ChangeView({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.kind !== 'string') return <Labelled value={value} />;
  const fields = Object.entries(value).filter(([k, v]) => k !== 'kind' && k !== 'reason' && k !== 'title' && !empty(v));
  return (
    <div className="shape arch-change">
      <div className="work-item-head">
        <span className="badge">{words(value.kind)}</span>
        {str(value.title) !== '' ? (
          <strong>
            <MarkdownInline text={str(value.title)} />
          </strong>
        ) : null}
      </div>
      <Markdown text={str(value.reason)} />
      {fields.length > 0 ? (
        <dl className="facts">
          {fields.map(([k, v]) => (
            <div key={k} className="fact">
              <dt>{label(k)}</dt>
              <dd>
                <FactValue value={v} />
              </dd>
            </div>
          ))}
        </dl>
      ) : null}
    </div>
  );
}

/// A part of an entry: its title, when and in which round, and what it holds.
function Part({ title, said, onRound, children }: { title: string; said: Said; onRound: OpenRound; children: ReactNode }) {
  return (
    <section className="arch-part">
      <div className="work-item-head">
        <span className="decision-label">{title}</span>
        <From said={said} onRound={onRound} />
      </div>
      {children}
    </section>
  );
}

/// One entry of an architect's log, folded under one line: a judgment of the whole, under its cell's title, with what
/// it handed in; or a matter it took, under its kind and what it is, with what each round of its path answered, and
/// whether a round closed it.
export function LogEntryView({ e, open, live, title, onRound }: {
  e: LogEntry;
  open: boolean;
  /// Whether the wake that took it still goes on.
  live: boolean;
  /// A cell's title in the graph.
  title: (cell: string) => string;
  onRound: OpenRound;
}) {
  if (e.kind === 'judgment') {
    return (
      <details className="boxed arch-entry" open={open}>
        <summary>
          <span className="work-item-head">
            <span className="cell-id">{e.cell}</span>
            <strong>{title(e.cell)}</strong>
            <From said={{ work: e.work, round: e.round, cell: e.cell, at: e.at, value: null }} onRound={onRound} />
          </span>
        </summary>
        <Labelled value={e.value} />
      </details>
    );
  }
  return (
    <details className="boxed arch-entry" open={open}>
      <summary>
        <span className="work-item-head">
          <span className="badge">{words(e.matter_kind)}</span>
          <strong>
            <MarkdownInline text={e.what} />
          </strong>
          {e.closed === null ? <span className="badge">{t(live ? 'log_handling' : 'log_not_closed')}</span> : null}
          <From said={{ work: e.work, round: e.round, cell: '', at: e.at, value: null }} onRound={onRound} />
        </span>
      </summary>
      <div className="arch-entry-body">
        {e.rounds.map((r) => (
          <Part key={r.round} title={`${r.cell} ${title(r.cell)}`} said={r} onRound={onRound}>
            <Labelled value={r.value} />
          </Part>
        ))}
      </div>
    </details>
  );
}

/// The fields of a line of work under its name, in the order they read, and the one folded under them.
const LINE_PARTS = ['serves', 'approach', 'where', 'stuck', 'recent'] as const;

/// One line of work as a block: its name as the heading, then its fields as short labelled paragraphs, how it knows
/// folded under them. Compared with the version before, it is marked new, changed or gone, and a changed field shows
/// what it was struck through beside what it is.
function LineBlock({ l, compare }: { l: LineChange; compare: boolean }) {
  const field = (name: string) => l.fields.find((f) => f.field === name);
  const said = (f: LineChange['fields'][number] | undefined) => {
    if (f === undefined) return null;
    if (compare && f.changed) {
      return (
        <>
          <del className="arch-was">
            <MarkdownInline text={f.before ?? ''} />
          </del>{' '}
          <ins className="arch-now">
            <MarkdownInline text={f.after ?? ''} />
          </ins>
        </>
      );
    }
    return <MarkdownInline text={(l.kind === 'removed' ? f.before : f.after) ?? ''} />;
  };
  const has = (f: LineChange['fields'][number] | undefined) =>
    f !== undefined && ((f.before ?? '') !== '' || (f.after ?? '') !== '');
  const marked = compare && l.kind !== 'same';
  const chip = l.kind === 'added' ? t('line_added') : l.kind === 'changed' ? t('line_changed') : t('line_removed');
  const grounds = field('grounds');
  return (
    <div className={marked ? `arch-block arch-line-${l.kind}` : 'arch-block'}>
      <div className="arch-block-h">
        {marked ? <span className={`badge arch-chip-${l.kind}`}>{chip}</span> : null}
        <strong>{said(field('line'))}</strong>
      </div>
      {LINE_PARTS.map((name) =>
        has(field(name)) ? (
          <p key={name} className="arch-block-field">
            <span className="arch-block-label">{t(`line_${name}`)}</span> {said(field(name))}
          </p>
        ) : null,
      )}
      {has(grounds) ? <Fold title={t('line_grounds')}>{said(grounds)}</Fold> : null}
    </div>
  );
}

/// A part of the understanding folded under its title and how many it holds, closed unless `open`; none when it holds
/// nothing.
function Section({ title, n, open = false, children }: { title: string; n?: number; open?: boolean; children: ReactNode }) {
  if (n === 0) return null;
  return <Fold title={n !== undefined ? t('understanding_count', { title, n }) : title} open={open}>{children}</Fold>;
}

/// Every version of the understanding, newest first, one at a time with the newer and older ones a click away: its
/// wake and round, each line of work as a block; with the switch on, each line marked new, changed or gone since the
/// version before it (a line written again in other words matched by what it says), the latest shown clean otherwise;
/// where it heads, open; and folded, what it says changed and what the wrap-ups after it learned.
function StateVersions({ u, onRound }: { u: Understanding; onRound: OpenRound }) {
  const [at, setAt] = useState(0);
  const [compare, setCompare] = useState(false);
  const versions = u.versions;
  const v = versions[Math.min(at, versions.length - 1)];
  if (v === undefined) return null;
  const k = Math.min(at, versions.length - 1);
  const before = versions[k + 1];
  const comparing = compare && before !== undefined;
  const c = stateChanges(before !== undefined ? before.state : null, v.state);
  const said: Said = { work: v.work, round: v.round, cell: v.cell, at: v.at, value: v.state };
  const lines = c.lines.filter((l) => comparing || l.kind !== 'removed');
  const learned = Array.isArray(v.learned) ? v.learned.filter((x) => str(x) !== '') : [];
  return (
    <section className="arch-part">
      <div className="work-item-head">
        <span className="decision-label">{t('understanding_state')}</span>
        <From said={said} onRound={onRound} />
        <span className="arch-versions">
          <button type="button" className="link" disabled={k === 0} onClick={() => setAt(k - 1)}>
            {t('understanding_newer')}
          </button>
          <span className="muted">{t('understanding_version', { k: versions.length - k, n: versions.length })}</span>
          <button type="button" className="link" disabled={k === versions.length - 1} onClick={() => setAt(k + 1)}>
            {t('understanding_older')}
          </button>
        </span>
      </div>
      {before !== undefined ? (
        <label className="arch-compare">
          <input type="checkbox" checked={compare} onChange={(e) => setCompare(e.target.checked)} />{' '}
          {t('understanding_compare')}
        </label>
      ) : null}
      {comparing ? (
        <p className="muted arch-since">{t('understanding_since', { round: `R${before.work}-${before.round}` })}</p>
      ) : null}
      {lines.map((l, i) => (
        <LineBlock key={i} l={l} compare={comparing} />
      ))}
      {isObj(v.state) && str(v.state.direction) !== '' ? (
        <Section title={t('understanding_direction')} open>
          <Markdown text={str(v.state.direction)} />
        </Section>
      ) : null}
      {c.changed.after !== '' ? (
        <Section title={t('understanding_changed')}>
          <Markdown text={c.changed.after} />
        </Section>
      ) : null}
      <Section title={t('understanding_learned_wake')} n={learned.length}>
        <ul className="arch-list">
          {learned.map((x, i) => (
            <li key={i}>
              <MarkdownInline text={str(x)} />
            </li>
          ))}
        </ul>
      </Section>
    </section>
  );
}

/// Where the project stands, as its architect last wrote it: its understanding with its versions; then, each folded
/// under its title and how many it holds, the notes written since, where the facts no longer match an earlier judgment
/// and what it could not settle, each as blocks; and the policy's check against the constitution, its counts by status
/// in its title, the articles broken and at risk first.
export function UnderstandingView({ u, onRound }: { u: Understanding; onRound: OpenRound }) {
  if (u.state === null && u.against === null) return <p className="muted">{t('understanding_none')}</p>;
  const state = u.state !== null && isObj(u.state.value) ? u.state.value : null;
  const against = u.against !== null && isObj(u.against.value) ? u.against.value : null;
  const rows = againstInOrder(against?.against ?? null);
  const mismatches = objs(state?.mismatches);
  const unclear = objs(state?.unclear);
  const notes = objs(u.notes);
  const count = (status: string) => rows.filter((r) => str(r.status) === status).length;
  const counts = (
    [
      ['broken', 'against_broken'],
      ['at_risk', 'against_at_risk'],
      ['kept', 'against_kept'],
    ] as const
  )
    .filter(([s]) => count(s) > 0)
    .map(([s, key]) => t(key, { n: count(s) }));
  return (
    <>
      {u.versions.length > 0 ? <StateVersions u={u} onRound={onRound} /> : null}
      <Section title={t('understanding_notes')} n={notes.length}>
        {notes.map((n, i) => (
          <div key={i} className="arch-block">
            {n.item !== null && n.item !== undefined ? <strong>#{str(n.item)} </strong> : null}
            <MarkdownInline text={str(n.text)} />
          </div>
        ))}
      </Section>
      <Section title={t('understanding_mismatches')} n={mismatches.length}>
        {mismatches.map((m, i) => (
          <div key={i} className="arch-block">
            <div className="arch-block-h">
              <strong>
                <MarkdownInline text={str(m.what)} />
              </strong>
            </div>
            <Field label={t('mismatch_fact')} value={m.fact} />
            <Field label={t('mismatch_involved')} value={m.involved} />
          </div>
        ))}
      </Section>
      <Section title={t('understanding_unclear')} n={unclear.length}>
        {unclear.map((x, i) => (
          <div key={i} className="arch-block">
            <div className="arch-block-h">
              <strong>
                <MarkdownInline text={str(x.what)} />
              </strong>
            </div>
            <Markdown text={str(x.why)} />
          </div>
        ))}
      </Section>
      {u.against !== null ? (
        <Section title={[t('understanding_against'), ...counts].join(' · ')}>
          <From said={u.against} onRound={onRound} />
          {rows.map((r, i) => (
            <div key={i} className="arch-block">
              <div className="arch-block-h">
                <span className={`badge arch-${str(r.status)}`}>{words(str(r.status))}</span>
                <strong>{str(r.article)}</strong>
              </div>
              {str(r.where) !== '' ? (
                <p className="arch-block-field">
                  <span className="arch-block-label">{t('against_where')}</span> <MarkdownInline text={str(r.where)} />
                </p>
              ) : null}
              <Markdown text={str(r.grounds)} />
            </div>
          ))}
        </Section>
      ) : null}
    </>
  );
}
