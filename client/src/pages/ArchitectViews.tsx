// What the architect hands back, each drawn by its shape in the flow's formats: the policy, the study of a new one,
// its probes and the readings of it by a new architect; the changes it collects for the owner's approval; its answer to
// whether what runs fits; a matter it handled, with the crux, the judgment as the options compared, the changes and the
// second look; the state of things and the check against the constitution; the plan. A value not of the shape expected
// is shown as labelled text, never as a tree of JSON.

import { type ReactNode, useState } from 'react';

import type { JsonValue, LogEntry, PlanBudget, PlanGate, PlanSegment, Said, Understanding } from '../api';
import { againstInOrder, LINE_FIELDS, stateChanges } from '../architect';
import { time } from '../format';
import { Field, Fold, OptionBoxes } from '../item/Shapes';
import { empty, isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Link } from '../route';
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

/// The changes of a judgment, numbered, each with why and what it does.
export function ChangesList({ value }: { value: JsonValue }) {
  const list = Array.isArray(value) ? value : [];
  if (list.length === 0) return null;
  return (
    <ol className="arch-list arch-numbered">
      {list.map((c, i) => (
        <li key={i}>
          <ChangeView value={c} />
        </li>
      ))}
    </ol>
  );
}

/// Whether what runs and what starts next fit: the verdict in words, why, and each place to change.
export function FitView({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.verdict !== 'string') return <Labelled value={value} />;
  const fixes = objs(value.fixes);
  return (
    <div className="shape">
      <Field label={t('fit_why')} value={value.why} />
      {fixes.length > 0 ? (
        <ul className="arch-list">
          {fixes.map((f, i) => (
            <li key={i}>
              <strong>
                <MarkdownInline text={str(f.where)} />
              </strong>
              <Field label={t('fit_wrong')} value={f.wrong} />
              <Field label={t('fit_fix')} value={f.fix} />
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

/// The verdict on whether what runs fits, as a badge.
function FitBadge({ value }: { value: JsonValue }) {
  const v = isObj(value) ? str(value.verdict) : '';
  return <span className={`badge ${v === 'fits' ? 'verdict-good' : 'verdict-bad'}`}>{words(v)}</span>;
}

/// The crux of a matter: what it really has to answer, and whether its root lies deeper.
function CruxView({ value }: { value: JsonValue }) {
  if (!isObj(value)) return <Labelled value={value} />;
  const root = str(value.root);
  return (
    <div className="shape">
      <Markdown text={str(value.question)} />
      {root === 'itself' ? <p className="prose muted">{t('crux_itself')}</p> : null}
      {root === 'deeper' ? (
        <>
          <Field label={t('crux_deeper')} value={value.deeper} />
          <p className="prose muted">{t(value.handle === 'together' ? 'crux_together' : 'crux_separately')}</p>
        </>
      ) : null}
      {root === 'policy_must_change' ? <Field label={t('crux_policy')} value={value.why} /> : null}
    </div>
  );
}

/// A judgment: the options compared, the one chosen marked, each with what it does, what follows and why; the
/// articles it touches, the items, how it fits the policy and the plan, and what is inferred.
function JudgmentView({ value }: { value: JsonValue }) {
  const j = isObj(value) && isObj(value.judgment) ? value.judgment : null;
  if (j === null) return <Labelled value={value} />;
  const principles = objs(j.principles);
  const touches = objs(j.touches);
  return (
    <div className="shape">
      <OptionBoxes
        options={objs(j.options)}
        fields={[
          ['does', t('option_does')],
          ['result', t('judgment_result')],
          ['why', t('judgment_why')],
        ]}
        recommended={str(j.chosen)}
        mark={t('decision_chosen')}
      />
      {principles.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('judgment_principles')}</span>
          <ul className="arch-list">
            {principles.map((p, i) => (
              <li key={i}>
                <strong>{str(p.principle)}</strong>: <MarkdownInline text={str(p.implies)} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {touches.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('judgment_touches')}</span>
          <ul className="arch-list">
            {touches.map((x, i) => (
              <li key={i}>
                <Link to={`/items/${str(x.item)}`}>#{str(x.item)}</Link> <MarkdownInline text={str(x.how)} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <Field label={t('judgment_fit')} value={j.fit} />
      <Field label={t('judgment_inferred')} value={j.inferred} />
    </div>
  );
}

/// The second look at a judgment: why all four stand, or what does not.
function LookView({ value }: { value: JsonValue }) {
  if (!isObj(value)) return <Labelled value={value} />;
  const issues = objs(value.issues);
  return (
    <div className="shape">
      <div>
        <span className={`badge ${value.verdict === 'stands' ? 'verdict-good' : 'verdict-bad'}`}>
          {words(str(value.verdict))}
        </span>
      </div>
      <Field label={t('look_duplicates')} value={value.duplicates} />
      <Field label={t('look_place')} value={value.place} />
      <Field label={t('look_checked')} value={value.checked} />
      <Field label={t('look_fits')} value={value.fits} />
      {issues.length > 0 ? (
        <ul className="arch-list">
          {issues.map((x, i) => (
            <li key={i}>
              <span className="badge">{words(str(x.about))}</span> <MarkdownInline text={str(x.what)} />
              <Field label={t('look_fix')} value={x.fix} />
            </li>
          ))}
        </ul>
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

/// One entry of the architect's log, folded under one line: its answer to whether what runs fits; or a matter it
/// picked, under its kind and what it is, with why it was picked, the crux, the judgment it stands by (and those a
/// second look sent back, folded), the changes, and the second look.
export function LogEntryView({ e, open, live, onRound }: {
  e: LogEntry;
  open: boolean;
  /// Whether the wake that picked it still goes on.
  live: boolean;
  onRound: OpenRound;
}) {
  if (e.kind === 'fit') {
    return (
      <details className="boxed arch-entry" open={open}>
        <summary>
          <span className="work-item-head">
            <strong>{t('log_fit')}</strong>
            <FitBadge value={e.fit} />
            <From said={{ work: e.work, round: e.round, at: e.at, value: null }} onRound={onRound} />
          </span>
        </summary>
        <FitView value={e.fit} />
      </details>
    );
  }
  const chosen = isObj(e.pick) && isObj(e.pick.chosen) ? e.pick.chosen : {};
  const ranked = isObj(e.pick) ? objs(e.pick.ranked) : [];
  const judgment = e.judgments[e.judgments.length - 1] ?? null;
  const earlier = e.judgments.slice(0, -1);
  const look = e.looks[e.looks.length - 1] ?? null;
  const decided = judgment !== null && isObj(judgment.value) && isObj(judgment.value.judgment) ? str(judgment.value.judgment.chosen) : '';
  const changes = judgment !== null && isObj(judgment.value) ? judgment.value.changes : undefined;
  const n = Array.isArray(changes) ? changes.length : 0;
  return (
    <details className="boxed arch-entry" open={open}>
      <summary>
        <span className="work-item-head">
          {str(chosen.about) !== '' ? <span className="badge">{words(str(chosen.about))}</span> : null}
          <strong>
            <MarkdownInline text={str(chosen.matter)} />
          </strong>
          {decided !== '' ? <span className="muted">→ {decided}</span> : null}
          {e.recorded === null ? (
            <span className="badge">{t(live ? 'log_handling' : 'log_not_recorded')}</span>
          ) : null}
          <From said={{ work: e.work, round: e.round, at: e.at, value: null }} onRound={onRound} />
        </span>
      </summary>
      <div className="arch-entry-body">
        <Field label={t('log_picked_why')} value={chosen.why} />
        {ranked.length > 1 ? (
          <Fold title={t('log_ranked', { n: ranked.length })}>
            <ol className="arch-list">
              {ranked.map((r, i) => (
                <li key={i}>
                  <MarkdownInline text={str(r.matter)} /> <span className="muted">· {str(r.why)}</span>
                </li>
              ))}
            </ol>
          </Fold>
        ) : null}
        {e.crux !== null ? (
          <Part title={t('log_crux')} said={e.crux} onRound={onRound}>
            <CruxView value={e.crux.value} />
          </Part>
        ) : null}
        {judgment !== null ? (
          <Part title={t('log_judgment')} said={judgment} onRound={onRound}>
            <JudgmentView value={judgment.value} />
          </Part>
        ) : null}
        {n > 0 && judgment !== null ? (
          <Part title={t('log_changes', { n })} said={judgment} onRound={onRound}>
            <ChangesList value={changes ?? null} />
          </Part>
        ) : null}
        {look !== null ? (
          <Part title={t('log_look')} said={look} onRound={onRound}>
            <LookView value={look.value} />
          </Part>
        ) : null}
        {earlier.length > 0 ? (
          <Fold title={t('log_judged_again', { n: earlier.length })}>
            {earlier.map((j, i) => (
              <div key={j.round} className="arch-part">
                <From said={j} onRound={onRound} />
                <JudgmentView value={j.value} />
                {e.looks[i] !== undefined ? <LookView value={e.looks[i].value} /> : null}
              </div>
            ))}
          </Fold>
        ) : null}
      </div>
    </details>
  );
}

/// Every version of the state of things, newest first, one at a time with the newer and older ones a click away: its
/// wake and round, each line of work marked new, changed or gone since the version before it (what a changed field was
/// struck through, what it is now marked), what it says changed and what its wake learned; the first version is all
/// new.
function StateVersions({ u, onRound }: { u: Understanding; onRound: OpenRound }) {
  const [at, setAt] = useState(0);
  const versions = u.versions;
  const v = versions[Math.min(at, versions.length - 1)];
  if (v === undefined) return null;
  const k = Math.min(at, versions.length - 1);
  const before = versions[k + 1];
  const c = stateChanges(before !== undefined ? before.state : null, v.state);
  const said: Said = { work: v.work, round: v.round, at: v.at, value: v.state };
  const chip = (kind: string) =>
    kind === 'added' ? t('line_added') : kind === 'changed' ? t('line_changed') : kind === 'removed' ? t('line_removed') : '';
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
      <p className="muted arch-since">
        {before === undefined ? t('understanding_first') : t('understanding_since', { round: `R${before.work}-${before.round}` })}
      </p>
      {c.lines.length > 0 ? (
        <div className="arch-table-wrap">
          <table className="list arch-lines">
            <thead>
              <tr>
                <th />
                {LINE_FIELDS.map((f) => (
                  <th key={f}>{t(`line_${f}`)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {c.lines.map((l, i) => (
                <tr key={i} className={`arch-line-${l.kind}`}>
                  <td className="nowrap">{l.kind !== 'same' ? <span className={`badge arch-chip-${l.kind}`}>{chip(l.kind)}</span> : null}</td>
                  {l.fields.map((f) => (
                    <td key={f.field}>
                      {f.changed ? (
                        <>
                          <del className="arch-was">
                            <MarkdownInline text={f.before ?? ''} />
                          </del>{' '}
                          <ins className="arch-now">
                            <MarkdownInline text={f.after ?? ''} />
                          </ins>
                        </>
                      ) : (
                        <MarkdownInline text={(l.kind === 'removed' ? f.before : f.after) ?? ''} />
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      <Field label={t('understanding_changed')} value={c.changed.after} />
      <Texts label={t('understanding_learned_wake')} value={v.learned} />
    </section>
  );
}

/// Where things stand, as the architect last wrote it: each line of work as a row of a table, what changed and what
/// was learned since; then the articles of the constitution, those broken and at risk first and those kept folded, and
/// where its earlier judgments no longer match.
export function UnderstandingView({ u, onRound }: { u: Understanding; onRound: OpenRound }) {
  if (u.state === null && u.against === null) return <p className="muted">{t('understanding_none')}</p>;
  const state = u.state !== null && isObj(u.state.value) ? u.state.value : null;
  const lines = state !== null ? objs(state.lines) : [];
  const against = u.against !== null && isObj(u.against.value) ? u.against.value : null;
  const rows = againstInOrder(against?.against ?? null);
  const kept = rows.filter((r) => r.status === 'kept');
  const notKept = rows.filter((r) => r.status !== 'kept');
  const mismatches = objs(against?.mismatches);
  const cols = ['line', 'serves', 'approach', 'where', 'stuck', 'recent'] as const;
  const article = (r: Obj, i: number) => (
    <li key={i}>
      <span className={`badge arch-${str(r.status)}`}>{words(str(r.status))}</span> <strong>{str(r.principle)}</strong>
      {str(r.where) !== '' ? (
        <span className="muted" title={t('against_where')}>
          {' · '}
          <MarkdownInline text={str(r.where)} />
        </span>
      ) : null}
      <Markdown text={str(r.evidence)} />
    </li>
  );
  return (
    <>
      {u.versions.length > 0 ? (
        <StateVersions u={u} onRound={onRound} />
      ) : u.state !== null ? (
        <section className="arch-part">
          <div className="work-item-head">
            <span className="decision-label">{t('understanding_state')}</span>
            <From said={u.state} onRound={onRound} />
          </div>
          {lines.length > 0 ? (
            <div className="arch-table-wrap">
              <table className="list arch-lines">
                <thead>
                  <tr>
                    {cols.map((c) => (
                      <th key={c}>{t(`line_${c}`)}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {lines.map((l, i) => (
                    <tr key={i}>
                      {cols.map((c) => (
                        <td key={c}>
                          <MarkdownInline text={str(l[c])} />
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
          {state !== null ? <Field label={t('understanding_changed')} value={state.changed} /> : null}
          <Texts label={t('understanding_learned')} value={u.learned} />
        </section>
      ) : null}
      {u.against !== null ? (
        <section className="arch-part">
          <div className="work-item-head">
            <span className="decision-label">{t('understanding_against')}</span>
            <From said={u.against} onRound={onRound} />
          </div>
          {notKept.length > 0 ? <ul className="arch-list">{notKept.map(article)}</ul> : null}
          {kept.length > 0 ? (
            <Fold title={t('against_kept', { n: kept.length })}>
              <ul className="arch-list">{kept.map(article)}</ul>
            </Fold>
          ) : null}
          {mismatches.length > 0 ? (
            <div className="field">
              <span className="decision-label">{t('understanding_mismatches')}</span>
              <ul className="arch-list">
                {mismatches.map((m, i) => (
                  <li key={i}>
                    <Field label={t('mismatch_before')} value={m.before} />
                    <Field label={t('mismatch_now')} value={m.now} />
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </section>
      ) : null}
    </>
  );
}

/// One segment of the plan, folded under its key, title and stage: what it delivers, when it is done, what must come
/// first, what it rests on, what to watch, its size and its items with the cell each one's work is at.
function SegmentView({ s }: { s: PlanSegment }) {
  const items = objs(s.items);
  const assumptions = objs(s.assumptions);
  return (
    <details className="boxed">
      <summary>
        <span className="work-item-head">
          <code>{s.key}</code>
          <strong>{s.title}</strong>
          <span className="muted">
            {t('segment_stage', { stage: s.stage })} · {s.project} · {t('segment_items', { n: items.length })}
          </span>
          {s.deferred !== null ? <span className="badge">{t('segment_deferred', { until: s.deferred })}</span> : null}
        </span>
      </summary>
      <div className="shape">
        <Field label={t('segment_delivers')} value={s.delivers} />
        <Field label={t('segment_done_when')} value={s.done_when} />
        <Texts label={t('segment_after')} value={s.after} />
        {assumptions.length > 0 ? (
          <div className="field">
            <span className="decision-label">{t('segment_assumptions')}</span>
            <ul className="arch-list">
              {assumptions.map((a, i) => (
                <li key={i}>
                  <MarkdownInline text={str(a.holds)} />{' '}
                  <span className="muted">· {t('segment_wrong_if', { what: str(a.wrong_if) })}</span>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
        <Texts label={t('segment_indicators')} value={s.indicators} />
        <Field label={t('segment_size')} value={s.size} />
        {items.length > 0 ? (
          <ul className="arch-list">
            {items.map((x, i) => (
              <li key={i}>
                <Link to={`/items/${str(x.item)}`}>
                  #{str(x.item)} {str(x.title)}
                </Link>
                {x.at !== null && x.at !== undefined ? <span className="muted"> · {t('segment_item_at', { cell: str(x.at) })}</span> : null}
                {x.held !== null && x.held !== undefined ? <span className="badge">{t('segment_item_held')}</span> : null}
                {x.cancelled !== null && x.cancelled !== undefined ? (
                  <span className="badge">{t('segment_item_cancelled')}</span>
                ) : null}
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </details>
  );
}

/// The plan: its segments in the order works start from them, then its gates and budgets, folded.
export function PlanView({ segments, gates, budgets }: { segments: PlanSegment[]; gates: PlanGate[]; budgets: PlanBudget[] }) {
  if (segments.length === 0 && gates.length === 0 && budgets.length === 0) {
    return <p className="muted">{t('plan_none')}</p>;
  }
  return (
    <>
      {segments.map((s) => (
        <SegmentView key={s.key} s={s} />
      ))}
      {gates.length > 0 ? (
        <Fold title={t('plan_gates', { n: gates.length })}>
          <ul className="arch-list">
            {gates.map((g) => (
              <li key={g.name}>
                <strong>{g.name}</strong> <span className="muted">· {words(g.what)}</span>
                <Field label={t('plan_basis')} value={g.basis} />
                <Texts label={t('plan_gate_segments')} value={g.segments} />
              </li>
            ))}
          </ul>
        </Fold>
      ) : null}
      {budgets.length > 0 ? (
        <Fold title={t('plan_budgets', { n: budgets.length })}>
          <ul className="arch-list">
            {budgets.map((b) => (
              <li key={b.name}>
                <strong>{b.name}</strong> <span className="muted">· {b.line}</span>
                <Field label={t('plan_measure')} value={b.measure} />
                <Field label={t('plan_basis')} value={b.basis} />
                <ul className="arch-list">
                  {objs(b.split).map((x, i) => (
                    <li key={i}>
                      <code>{str(x.segment)}</code> {str(x.amount)}
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        </Fold>
      ) : null}
    </>
  );
}
