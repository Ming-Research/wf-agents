// What an architect hands back, each drawn by its shape in the flow's formats and in the page's shared parts (blocks
// under who wrote them): the policy, and an item change its batch made.
// A value not of the shape expected is shown as labelled text, never as a tree of JSON.

import { Label } from '@primer/react';
import { type ReactNode } from 'react';

import type { JsonValue } from '../api';
import { DocList } from '../document';
import { Field } from '../item/Shapes';
import { empty, isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, SubHeading } from '../parts';
import { t } from '../text';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

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

