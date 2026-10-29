// What an architect hands back, each drawn by its shape in the flow's formats and in the page's shared parts (blocks
// under who wrote them, sections that fold): the policy, the study of a new one, its probes and the readings of it by
// a new architect; an item change its batch made; its understanding of the project with its versions, the check of the
// policy against the constitution and of whether what runs fits. A value not of the shape expected is shown as
// labelled text, never as a tree of JSON.

import { type ReactNode, useState } from 'react';

import type { JsonValue, Said, Understanding } from '../api';
import { againstInOrder, type LineChange, stateChanges } from '../architect';
import { Field } from '../item/Shapes';
import { empty, isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, BlockField, By, type OpenRound, Section } from '../parts';
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

/// Who wrote a value the architect handed back: its cell, when, and its round.
function wroteBy(s: Said, onRound: OpenRound): ReactNode {
  return <By cell={s.cell} at={s.at} work={s.work} round={s.round} onRound={onRound} />;
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
        <Section title={t('study_basis')} n={basis.length}>
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
        </Section>
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

/// A new architect's reading of the policy, as a block under who read it (`by`), folded under the verdict on it: its
/// restatement point by point, what it asked, and its answers to the probes.
export function ReadingView({ by, reading, verdict }: { by: ReactNode; reading: JsonValue; verdict: JsonValue }) {
  const r = isObj(reading) ? reading : {};
  const restate = objs(r.restate);
  const answers = objs(r.answers);
  return (
    <Block head={<strong>{t('reading_head')}</strong>} by={by}>
      <ReadingVerdict value={verdict} />
      <Section title={t('reading_text')}>
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
      </Section>
    </Block>
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

/// One change to the database the architect collected, as a block: in its head, what `before` says of it, its kind
/// and, for what it creates, the title, and who handed it in (`by`); then why, what it does, each field beside its
/// name, and what `children` add.
export function ChangeBlock({ value, before, by, children }: {
  value: JsonValue;
  before: ReactNode;
  by: ReactNode;
  children?: ReactNode;
}) {
  if (!isObj(value) || typeof value.kind !== 'string') {
    return (
      <Block head={before} by={by}>
        <Labelled value={value} />
        {children}
      </Block>
    );
  }
  const fields = Object.entries(value).filter(([k, v]) => k !== 'kind' && k !== 'reason' && k !== 'title' && !empty(v));
  const head = (
    <>
      {before}
      <span className="badge">{words(value.kind)}</span>
      {str(value.title) !== '' ? (
        <strong>
          <MarkdownInline text={str(value.title)} />
        </strong>
      ) : null}
    </>
  );
  return (
    <Block head={head} by={by}>
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
      {children}
    </Block>
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
  const head = (
    <>
      {marked ? <span className={`badge arch-chip-${l.kind}`}>{chip}</span> : null}
      <strong>{said(field('line'))}</strong>
    </>
  );
  return (
    <Block head={head} tone={marked ? `arch-line-${l.kind}` : undefined}>
      {LINE_PARTS.map((name) =>
        has(field(name)) ? (
          <BlockField key={name} label={t(`line_${name}`)}>
            {said(field(name))}
          </BlockField>
        ) : null,
      )}
      {has(grounds) ? <Section title={t('line_grounds')}>{said(grounds)}</Section> : null}
    </Block>
  );
}

/// Every version of the understanding, newest first, one at a time with the newer and older ones a click away: its
/// wake and round, each line of work as a block; with the switch on, each line marked new, changed or gone since the
/// version before it (a line written again in other words matched by what it says), the latest shown clean otherwise;
/// where it heads, open; and folded, what it says changed; each part naming the cell and round that wrote it.
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
  const from = wroteBy(said, onRound);
  return (
    <Section title={t('understanding_state')} open by={from}>
      <span className="arch-versions">
        <button type="button" className="link" disabled={k === 0} onClick={() => setAt(k - 1)}>
          {t('understanding_newer')}
        </button>
        <span className="muted">{t('understanding_version', { k: versions.length - k, n: versions.length })}</span>
        <button type="button" className="link" disabled={k === versions.length - 1} onClick={() => setAt(k + 1)}>
          {t('understanding_older')}
        </button>
      </span>
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
        <Section title={t('understanding_direction')} open by={from}>
          <Markdown text={str(v.state.direction)} />
        </Section>
      ) : null}
      {c.changed.after !== '' ? (
        <Section title={t('understanding_changed')} by={from}>
          <Markdown text={c.changed.after} />
        </Section>
      ) : null}
    </Section>
  );
}

/// The kinds of a place to set straight the check of what runs found, in words.
function FitPlace({ fix, matter, onMatter }: { fix: Obj; matter: number | undefined; onMatter: (id: number) => void }) {
  const steps = texts(fix.steps);
  const head = (
    <>
      <span className="badge">{words(str(fix.kind))}</span>
      <strong>
        <MarkdownInline text={str(fix.where)} />
      </strong>
      {fix.pause_now === true ? <span className="badge verdict-bad">{t('fit_paused')}</span> : null}
      {matter !== undefined ? (
        <button type="button" className="link" onClick={() => onMatter(matter)}>
          {t('fit_matter', { id: matter })}
        </button>
      ) : null}
    </>
  );
  return (
    <Block head={head}>
      <Markdown text={str(fix.wrong)} />
      {steps.length > 0 ? (
        <ol className="arch-list">
          {steps.map((x, i) => (
            <li key={i}>
              <MarkdownInline text={x} />
            </li>
          ))}
        </ol>
      ) : null}
    </Block>
  );
}

/// Where the project stands, as the stage of the architect's that understands it last wrote it, each part naming the
/// cell and round that wrote it: the state of things with its versions; then, each folded under its title and how many
/// it holds, where the facts no longer match an earlier judgment and what it could not settle, as blocks; the policy's
/// check against the constitution, its counts by status in its title, the articles broken and at risk first; and the
/// check of whether what runs fits, its verdict and why, and each place to set straight with its steps and the matter
/// it became, which opens in the queue or the log.
export function UnderstandingView({ u, onRound, onMatter }: {
  u: Understanding;
  onRound: OpenRound;
  onMatter: (id: number) => void;
}) {
  if (u.state === null && u.against === null && u.fit === null) return <p className="muted">{t('understanding_none')}</p>;
  const state = u.state !== null && isObj(u.state.value) ? u.state.value : null;
  const against = u.against !== null && isObj(u.against.value) ? u.against.value : null;
  const rows = againstInOrder(against?.against ?? null);
  const mismatches = objs(state?.mismatches);
  const unclear = objs(state?.unclear);
  const fromState = u.state !== null ? wroteBy(u.state, onRound) : undefined;
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
  const fit = u.fit !== null && isObj(u.fit.value) && isObj(u.fit.value.fit) ? u.fit.value.fit : null;
  const fixes = objs(fit?.fixes);
  const verdict = fit !== null ? words(str(fit.verdict)) : '';
  return (
    <>
      {u.versions.length > 0 ? <StateVersions u={u} onRound={onRound} /> : null}
      <Section title={t('understanding_mismatches')} n={mismatches.length} by={fromState}>
        {mismatches.map((m, i) => (
          <Block
            key={i}
            head={
              <strong>
                <MarkdownInline text={str(m.what)} />
              </strong>
            }
          >
            <Field label={t('mismatch_fact')} value={m.fact} />
            <Field label={t('mismatch_involved')} value={m.involved} />
          </Block>
        ))}
      </Section>
      <Section title={t('understanding_unclear')} n={unclear.length} by={fromState}>
        {unclear.map((x, i) => (
          <Block
            key={i}
            head={
              <strong>
                <MarkdownInline text={str(x.what)} />
              </strong>
            }
          >
            <Markdown text={str(x.why)} />
          </Block>
        ))}
      </Section>
      {u.against !== null ? (
        <Section title={[t('understanding_against'), ...counts].join(' · ')} by={wroteBy(u.against, onRound)}>
          {rows.map((r, i) => (
            <Block
              key={i}
              head={
                <>
                  <span className={`badge arch-${str(r.status)}`}>{words(str(r.status))}</span>
                  <strong>{str(r.article)}</strong>
                </>
              }
            >
              {str(r.where) !== '' ? (
                <BlockField label={t('against_where')}>
                  <MarkdownInline text={str(r.where)} />
                </BlockField>
              ) : null}
              <Markdown text={str(r.grounds)} />
            </Block>
          ))}
        </Section>
      ) : null}
      {u.fit !== null && fit !== null ? (
        <Section
          title={t('understanding_fit', { verdict })}
          n={fixes.length > 0 ? fixes.length : undefined}
          by={wroteBy(u.fit, onRound)}
        >
          {str(fit.why) !== '' ? <Markdown text={str(fit.why)} /> : null}
          {fixes.map((fix, i) => (
            <FitPlace key={i} fix={fix} matter={u.fit_matters[i]} onMatter={onMatter} />
          ))}
        </Section>
      ) : null}
    </>
  );
}
