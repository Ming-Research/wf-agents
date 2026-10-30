// What an architect hands back, each drawn by its shape in the flow's formats and in the page's shared parts (blocks
// under who wrote them, sections that fold): the policy, and the check of the policy in force; and an item change its
// batch made.
// A value not of the shape expected is shown as labelled text, never as a tree of JSON.

import { Label, type LabelColorOptions } from '@primer/react';
import { type ReactNode } from 'react';

import type { JsonValue, Said } from '../api';
import { againstInOrder } from '../architect';
import { DocList } from '../document';
import { Field } from '../item/Shapes';
import { empty, isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, BlockField, By, type OpenRound, Section, SubHeading } from '../parts';
import { t } from '../text';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

/// Who wrote a value the architect handed back: its cell, when, and its round.
function wroteBy(s: Said, onRound: OpenRound): ReactNode {
  return <By cell={s.cell} at={s.at} work={s.work} round={s.round} onRound={onRound} />;
}

/// A policy, as one document: its direction and why, its principles numbered, and the signals that the direction may
/// have to change, each with how it shows.
export function PolicyText({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.direction !== 'string') return <Labelled value={value} />;
  const principles = objs(value.principles);
  const signals = objs(value.signals);
  return (
    <div className="document">
      <Field label={t('policy_direction')} value={value.direction} />
      <Field label={t('policy_why')} value={value.why} />
      {principles.length > 0 ? (
        <div className="field">
          <SubHeading>{t('policy_principles')}</SubHeading>
          <DocList
            items={principles.map((p, i) => ({
              lead: str(p.text),
              parts: [],
              no: typeof p.no === 'number' ? p.no : i + 1,
            }))}
          />
        </div>
      ) : null}
      {signals.length > 0 ? (
        <div className="field">
          <SubHeading>{t('policy_signals')}</SubHeading>
          <DocList items={signals.map((s) => ({ lead: str(s.what), parts: [[t('policy_signal_how'), s.how]] }))} />
        </div>
      ) : null}
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
      <Label variant="secondary">{words(value.kind)}</Label>
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

/// The colour of where the policy stands against an article of the constitution.
const AGAINST_TONE: Record<string, LabelColorOptions> = { kept: 'success', at_risk: 'attention', broken: 'danger' };

/// The latest check of whether the policy in force still holds, under the cell and round that made it: its verdict
/// and why, or each reason it may no longer hold; then the policy against the constitution, its counts by status in
/// its title, the articles broken and at risk first.
export function PolicyCheck({ check, onRound }: { check: Said; onRound: OpenRound }) {
  const value = isObj(check.value) ? check.value : null;
  const rows = againstInOrder(value?.against ?? null);
  const verdict = value !== null && isObj(value.verdict) ? value.verdict : null;
  const reasons = objs(verdict?.reasons);
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
    <Section
      title={t('policy_check', { verdict: verdict !== null ? words(str(verdict.verdict)) : '' })}
      open
      by={wroteBy(check, onRound)}
    >
      {verdict !== null && str(verdict.why) !== '' ? <Markdown text={str(verdict.why)} /> : null}
      {reasons.map((r, i) => (
        <Block
          key={i}
          head={
            <>
              <Label variant="secondary">{words(str(r.kind))}</Label>
              <strong>
                <MarkdownInline text={str(r.seen)} />
              </strong>
            </>
          }
        >
          <Markdown text={str(r.why)} />
        </Block>
      ))}
      <Section title={[t('understanding_against'), ...counts].join(' · ')} n={rows.length}>
        {rows.map((r, i) => (
          <Block
            key={i}
            head={
              <>
                <Label variant={AGAINST_TONE[str(r.status)] ?? 'secondary'}>{words(str(r.status))}</Label>
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
    </Section>
  );
}
