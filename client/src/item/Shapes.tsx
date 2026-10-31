// What an item's works hand back, each drawn by its shape in the flow's formats: the engineer's understanding and
// what it rests on, the approaches compared, the measure, the design, the acceptance and how it was met, the
// deviations. A value not of the shape expected is shown as labelled text, never as a tree of JSON.

import { Label, type LabelColorOptions } from '@primer/react';
import type { ReactNode } from 'react';

import type { JsonValue } from '../api';
import { DocList } from '../document';
import { empty, isObj, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Section, SubHeading } from '../parts';
import { t, type Key } from '../text';

/// A labelled text, when there is any: its name as a sub-heading over it.
export function Field({ label, value }: { label: string; value: JsonValue | undefined }) {
  if (empty(value)) return null;
  return (
    <div className="field">
      <SubHeading>{label}</SubHeading>
      {typeof value === 'string' ? <Markdown text={value} /> : <Labelled value={value} />}
    </div>
  );
}

/// A ground checked is done; one inferred waits for its check.
const GROUND_TONE: Record<string, LabelColorOptions> = { verified: 'success', inferred: 'attention' };

/// What a claim rests on: each verified or inferred, the claim, and where it was checked, or what it follows from and
/// how it will be confirmed.
export function Grounds({ value }: { value: JsonValue }) {
  if (!Array.isArray(value) || !value.every(isObj)) return <Labelled value={value} />;
  return (
    <ul className="grounds">
      {value.filter(isObj).map((g, i) => (
        <li key={i}>
          <Label variant={GROUND_TONE[str(g.kind)] ?? 'secondary'}>{words(str(g.kind))}</Label>{' '}
          <MarkdownInline text={str(g.claim)} />
          <Field label={t('ground_where')} value={g.where} />
          <Field label={t('ground_from')} value={g.from} />
          <Field label={t('ground_confirm')} value={g.confirm} />
        </li>
      ))}
    </ul>
  );
}

/// The engineer's understanding of the problem: what must be solved, why, its cause, and where it differs from the
/// item as written; what it rests on, folded.
export function Understanding({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.problem !== 'string') return <Labelled value={value} />;
  const grounds = Array.isArray(value.grounds) ? value.grounds : [];
  return (
    <div className="shape">
      <Markdown text={value.problem} />
      <Field label={t('problem_why')} value={value.why} />
      <Field label={t('problem_cause')} value={value.cause} />
      <Field label={t('problem_vs_item')} value={value.vs_item} />
      {grounds.length > 0 ? (
        <Section title={t('problem_grounds')} n={grounds.length}>
          <Grounds value={value.grounds} />
        </Section>
      ) : null}
    </div>
  );
}

/// Whether the choice rests on a measurement, in one line.
export function Measure({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.verdict !== 'string') return <Labelled value={value} />;
  if (value.verdict === 'not_measured') return <p className="prose muted">{t('measure_not', { why: str(value.why) })}</p>;
  if (value.verdict === 'measured') return <p className="prose muted">{t('measure_done', { effect: str(value.effect) })}</p>;
  return <Labelled value={value} />;
}

/// The design itself: how it solves the problem, open, with what follows it (the measure); where it changes, what it
/// leaves out and its cost and pitfalls, each folded.
export function Design({ value, after }: { value: JsonValue; after: ReactNode }) {
  if (!isObj(value) || typeof value.solution !== 'string') {
    return (
      <>
        <Labelled value={value} />
        {after}
      </>
    );
  }
  const folded: Array<[Key, JsonValue | undefined]> = [
    ['design_where', value.where],
    ['design_excluded', value.excluded],
    ['design_pitfalls', value.pitfalls],
  ];
  return (
    <div className="shape">
      <Markdown text={value.solution} />
      {after}
      {folded.map(([key, v]) =>
        empty(v) ? null : (
          <Section key={key} title={t(key)}>
            <Labelled value={v} />
          </Section>
        ),
      )}
    </div>
  );
}

/// The approaches compared, as a list of the design's document: each by its name, with how it works, what is good and
/// what is bad about it, the one chosen marked.
export function Approaches({ value, chosen }: { value: JsonValue; chosen: string }) {
  if (!Array.isArray(value) || !value.every((o) => isObj(o) && typeof o.name === 'string')) {
    return <Labelled value={value} />;
  }
  return (
    <DocList
      items={value.filter(isObj).map((o) => ({
        lead: str(o.name),
        mark: str(o.name) !== '' && str(o.name) === chosen ? <Label variant="success">{t('decision_chosen')}</Label> : undefined,
        parts: [
          [t('option_how'), o.how],
          [t('option_good'), o.good],
          [t('option_bad'), o.bad],
        ],
      }))}
    />
  );
}

/// Approaches or options, each in a box of its own, one under another: its name, then each of its fields under its
/// label, in order. The one recommended and the one the owner picked are marked on their boxes, and may be the same;
/// with `onPick`, a click on a box picks it and a second click clears it.
export function OptionBoxes(props: {
  options: Obj[];
  /// The fields under each option's name, each with its label, and with another for the option recommended.
  fields: Array<[string, string] | [string, string, string]>;
  recommended: string;
  /// What the recommended option is called here.
  mark: string;
  picked?: string;
  onPick?: ((name: string) => void) | null;
}) {
  const pick = props.onPick ?? null;
  return (
    <div className="option-boxes" role={pick !== null ? 'radiogroup' : undefined}>
      {props.options.map((o, i) => {
        const name = str(o.name);
        const recommended = name !== '' && name === props.recommended;
        const mine = name !== '' && name === props.picked;
        const cls = ['option-box', recommended ? 'recommended' : '', mine ? 'picked' : '', pick !== null ? 'pickable' : '']
          .filter((c) => c !== '')
          .join(' ');
        return (
          <div
            key={i}
            className={cls}
            onClick={pick !== null ? () => pick(name) : undefined}
            onKeyDown={
              pick !== null
                ? (e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      pick(name);
                    }
                  }
                : undefined
            }
            tabIndex={pick !== null ? 0 : undefined}
            role={pick !== null ? 'radio' : undefined}
            aria-checked={pick !== null ? mine : undefined}
          >
            <div className="option-head">
              <strong>
                <MarkdownInline text={name} />
              </strong>
              {recommended ? <Label variant="success">{props.mark}</Label> : null}
              {mine ? <Label variant="accent">{t('option_yours')}</Label> : null}
            </div>
            {props.fields.map(([k, label, ifRecommended]) => (
              <Field key={k} label={recommended && ifRecommended !== undefined ? ifRecommended : label} value={o[k]} />
            ))}
          </div>
        );
      })}
    </div>
  );
}

/// How an acceptance condition was met, as the work said at its end: by which tests, by which measurement, or where
/// the report answers it.
export type Met = Obj;

function MetLine({ m }: { m: Met }) {
  const how = str(m.how);
  if (how === 'tested') {
    const tests = Array.isArray(m.tests) ? m.tests.map((x) => str(x)) : [];
    return <span className="good">{t('met_tested', { tests: tests.join(', ') })}</span>;
  }
  if (how === 'measured') return <span className="good">{t('met_measured', { run: str(m.run), criterion: str(m.criterion) })}</span>;
  if (how === 'report_answer') return <span className="good">{t('met_report', { where: str(m.where) })}</span>;
  return <Labelled value={m} />;
}

/// The acceptance: each condition with how it is checked, and how it was met when the work has said.
export function Acceptance({ value, met }: { value: JsonValue; met: Map<string, Met> }) {
  if (!Array.isArray(value) || !value.every((r) => isObj(r) && typeof r.condition === 'string')) {
    return <Labelled value={value} />;
  }
  return (
    <table className="list acceptance">
      <tbody>
        {value.filter(isObj).map((r, i) => {
          const id = str(r.id);
          const by = str(r.by);
          const m = met.get(id);
          return (
            <tr key={i}>
              <td className="nowrap">
                <code>{id !== '' ? id : i + 1}</code>
              </td>
              <td>
                <Markdown text={str(r.condition)} />
                {m !== undefined ? <MetLine m={m} /> : null}
              </td>
              <td className="muted acceptance-by">
                {by === 'test'
                  ? t('acc_by_test')
                  : by === 'measurement'
                    ? t('acc_by_measurement', { criterion: str(r.criterion) })
                    : by === 'report_answer'
                      ? t('acc_by_report')
                      : words(by)}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/// Where a round departed from the design: its kind, what, and why.
export function Deviation({ value }: { value: JsonValue }) {
  if (!isObj(value) || typeof value.kind !== 'string') return <Labelled value={value} />;
  return (
    <div className="deviation">
      <Label variant="secondary">{words(value.kind)}</Label> <MarkdownInline text={str(value.what)} />
      <Field label={t('deviation_why')} value={value.why} />
    </div>
  );
}
