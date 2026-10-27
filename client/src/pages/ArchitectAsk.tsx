// An ask the architect put to the owner, as the owner reads and answers it, by its kind: a question, laid out as an
// engineer's ask is; an open choice, which holds no wake; the study of a new policy to read; the policy to approve, with
// its probes and every reading of it by a new architect; and the changes it collected, numbered, to approve. While it
// waits, its form: an option clicked and words; one of the choices its cell offers (satisfied, study on or no change;
// approved or not good enough) and words, needed unless the owner agrees; all, some (ticked) or none of the changes,
// and why. Each answer goes to the interface's answer of an ask, and the engine goes on.

import { useState } from 'react';

import type { JsonValue, OwnerAsk } from '../api';
import { type ChangesChoice, changesAnswer, choicesOf, rulingAnswer } from '../architect';
import { roundName, time } from '../format';
import { postJson } from '../http';
import { Field, Fold, OptionBoxes } from '../item/Shapes';
import { isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { t, type Key } from '../text';
import { ChangeView, PolicyText, ProbesView, ReadingView, StudyView } from './ArchitectViews';
import { AnswerForm, AskView, decided } from './Decide';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

const KINDS: Record<string, Key> = {
  ask: 'ask_kind_ask',
  open: 'ask_kind_open',
  study: 'ask_kind_study',
  policy_approval: 'ask_kind_policy_approval',
  changes: 'ask_kind_changes',
};

/// What an ask of the architect's is, in words, for its head.
export function askKindWords(a: OwnerAsk): string {
  const kind = isObj(a.body) ? str(a.body.kind) : '';
  const key = KINDS[kind];
  return key !== undefined ? t(key) : words(kind);
}

/// Sends an answer, and says what failed.
function useAnswer(a: OwnerAsk) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const send = (body: object) => {
    setBusy(true);
    setError(null);
    postJson(`/api/v1/asks/${a.id}/answer`, body).then(decided, (e: unknown) => {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return { busy, error, send };
}

/// The owner's answer once given: by whom and when, the choice, the changes approved, and the words.
function Answered({ a }: { a: OwnerAsk }) {
  const ans = isObj(a.answer) ? a.answer : {};
  const choice = str(ans.choice);
  const approved = Array.isArray(ans.approved) ? ans.approved.map((x) => str(x)) : [];
  return (
    <div className="decision-fact">
      <span className="decision-label">
        {t('ask_answer_by', { by: str(ans.by) })}
        {typeof ans.at === 'number' ? ` · ${time(ans.at)}` : ''}
      </span>
      {choice !== '' ? (
        <p className="prose">
          <strong>{label(choice)}</strong>
          {choice === 'approve_some' ? ` · ${t('changes_approved_list', { list: approved.join(', ') })}` : ''}
        </p>
      ) : null}
      {str(ans.text) !== '' ? <Markdown text={str(ans.text)} /> : null}
    </div>
  );
}

/// An open choice: the question, the options with the one recommended, how sure the architect is, and what it leaves
/// alone until the owner answers; it holds no wake.
function OpenAsk({ a, body }: { a: OwnerAsk; body: Obj }) {
  const [picked, setPicked] = useState('');
  const choice = isObj(body.choice) ? body.choice : {};
  const options = objs(choice.options);
  const affects = Array.isArray(body.affects) ? body.affects.map((x) => str(x)) : [];
  const waits = a.answer === null;
  return (
    <div className="card-view">
      <p className="muted">{t('ask_open_note')}</p>
      <p className="ask-question">{str(choice.question)}</p>
      <OptionBoxes
        options={options}
        fields={[
          ['does', t('option_does')],
          ['cost_risk', t('option_cost')],
        ]}
        recommended={str(choice.recommended)}
        mark={t('ask_recommended')}
        picked={waits ? picked : ''}
        onPick={waits ? (name) => setPicked((p) => (p === name ? '' : name)) : null}
      />
      {choice.confidence !== undefined ? (
        <Field label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
      ) : null}
      {affects.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('ask_affects')}</span>
          <ul className="arch-list">
            {affects.map((x, i) => (
              <li key={i}>
                <MarkdownInline text={x} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {waits ? <AnswerForm a={a} picked={picked} options={options.length > 0} /> : <Answered a={a} />}
    </div>
  );
}

/// The owner's ruling on what the architect put: one of the choices its cell offers, each saying on hovering what it
/// means, and words, needed unless the owner agrees.
function RulingForm({ a }: { a: OwnerAsk }) {
  const choices = choicesOf(a.body);
  const [choice, setChoice] = useState<string | null>(null);
  const [text, setText] = useState('');
  const { busy, error, send } = useAnswer(a);
  const answer = rulingAnswer(choices, choice, text);
  const meaning = choices.find(([v]) => v === choice)?.[1] ?? '';
  const agrees = choice !== null && choice === choices[0]?.[0];
  return (
    <div className="decide">
      <div className="decide-row">
        {choices.map(([v, m]) => (
          <button
            key={v}
            type="button"
            className={`decide-button${choice === v ? ' on' : ''}`}
            aria-pressed={choice === v}
            title={m}
            onClick={() => setChoice((c) => (c === v ? null : v))}
            disabled={busy}
          >
            {label(v)}
          </button>
        ))}
      </div>
      {meaning !== '' ? <p className="muted pick-hint">{meaning}</p> : null}
      <textarea
        value={text}
        rows={3}
        placeholder={t(agrees ? 'ruling_words_optional' : 'ruling_words_needed')}
        onChange={(e) => setText(e.target.value)}
        disabled={busy}
      />
      <div className="decide-row">
        <button
          type="button"
          className="decide-button"
          onClick={() => answer !== null && send(answer)}
          disabled={busy || answer === null}
          title={answer === null ? t('ruling_not_ready') : undefined}
        >
          {t('answer_send')}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// The policy to approve: its text, its probes folded, and each reading of it by a new architect with the verdict.
function PolicyAsk({ a, body }: { a: OwnerAsk; body: Obj }) {
  const readings = objs(body.readings);
  const probes = Array.isArray(body.probes) ? body.probes.length : 0;
  return (
    <div className="card-view">
      <PolicyText value={body.policy ?? null} />
      {probes > 0 ? (
        <Fold title={t('policy_probes', { n: probes })}>
          <ProbesView value={body.probes ?? null} />
        </Fold>
      ) : null}
      {readings.length > 0 ? (
        <div className="field">
          <span className="decision-label">{t('policy_readings', { n: readings.length })}</span>
          {readings.map((r, i) => (
            <ReadingView
              key={i}
              round={typeof r.round === 'number' ? roundName(a.work, r.round) : ''}
              reading={r.reading ?? null}
              verdict={r.verdict ?? null}
            />
          ))}
        </div>
      ) : null}
      {a.answer === null ? <RulingForm a={a} /> : <Answered a={a} />}
    </div>
  );
}

/// The matter a change belongs to, and who waits for it, in words.
function MatterOf({ c }: { c: Obj }) {
  const m = c.matter;
  const what = isObj(m) ? str(m.matter ?? m.what) : typeof m === 'number' ? t('change_matter_n', { n: m }) : str(m);
  const waiting = c.waiting;
  return (
    <div className="muted arch-change-for">
      {what !== '' ? (
        <span>
          {t('change_for')} <MarkdownInline text={what} />
        </span>
      ) : null}
      {waiting !== undefined && waiting !== null && str(waiting) !== '' ? (
        <span>
          {' · '}
          {t('change_waiting')} {isObj(waiting) ? <Labelled value={waiting} /> : str(waiting)}
        </span>
      ) : null}
    </div>
  );
}

const CHANGES: Array<[ChangesChoice, Key]> = [
  ['approve_all', 'changes_all'],
  ['approve_some', 'changes_some'],
  ['approve_none', 'changes_none'],
];

/// The changes the architect collected, numbered, each with the matter it belongs to, who waits, its kind, why and what
/// it does; while the ask waits, the owner approves all, some (ticking which) or none, saying why for what is not
/// approved; once answered, each change says whether it was approved.
function ChangesAsk({ a, body }: { a: OwnerAsk; body: Obj }) {
  const changes = objs(body.changes);
  const all = changes.map((c, i) => (typeof c.n === 'number' ? c.n : i + 1));
  const [choice, setChoice] = useState<ChangesChoice | null>(null);
  const [picked, setPicked] = useState<number[]>([]);
  const [text, setText] = useState('');
  const { busy, error, send } = useAnswer(a);
  const answer = changesAnswer(choice, picked, all, text);
  const waits = a.answer === null;
  const given = isObj(a.answer) && Array.isArray(a.answer.approved) ? a.answer.approved : null;
  const ticking = waits && choice === 'approve_some';
  return (
    <div className="card-view">
      <p className="muted">{t('ask_changes_note', { n: changes.length })}</p>
      <ol className="arch-list arch-changes">
        {changes.map((c, i) => {
          const n = all[i];
          return (
            <li key={n} value={n}>
              {ticking ? (
                <label className="arch-tick">
                  <input
                    type="checkbox"
                    checked={picked.includes(n)}
                    onChange={(e) => setPicked((p) => (e.target.checked ? [...p, n] : p.filter((x) => x !== n)))}
                  />{' '}
                  {t('changes_tick', { n })}
                </label>
              ) : null}
              {given !== null ? (
                <span className={`badge ${given.includes(n) ? 'verdict-good' : 'verdict-bad'}`}>
                  {t(given.includes(n) ? 'changes_was_approved' : 'changes_not_approved')}
                </span>
              ) : null}
              <MatterOf c={c} />
              <ChangeView value={c.change ?? null} />
            </li>
          );
        })}
      </ol>
      {waits ? (
        <div className="decide">
          <div className="decide-row">
            {CHANGES.map(([v, k]) => (
              <button
                key={v}
                type="button"
                className={`decide-button${choice === v ? ' on' : ''}`}
                aria-pressed={choice === v}
                onClick={() => setChoice((c) => (c === v ? null : v))}
                disabled={busy}
              >
                {t(k)}
              </button>
            ))}
          </div>
          {choice === 'approve_some' ? <p className="muted pick-hint">{t('changes_some_hint')}</p> : null}
          <textarea
            value={text}
            rows={3}
            placeholder={t(choice === 'approve_all' ? 'ruling_words_optional' : 'changes_why_hint')}
            onChange={(e) => setText(e.target.value)}
            disabled={busy}
          />
          <div className="decide-row">
            <button
              type="button"
              className="decide-button"
              onClick={() => answer !== null && send(answer)}
              disabled={busy || answer === null}
              title={answer === null ? t('changes_not_ready') : undefined}
            >
              {t('answer_send')}
            </button>
          </div>
          {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
        </div>
      ) : (
        <Answered a={a} />
      )}
    </div>
  );
}

/// An ask of the architect's, by its kind.
export function ArchitectAsk({ a }: { a: OwnerAsk }) {
  const body = isObj(a.body) ? a.body : {};
  switch (str(body.kind)) {
    case 'open':
      return <OpenAsk a={a} body={body} />;
    case 'study':
      return (
        <div className="card-view">
          <StudyView value={body.study ?? null} />
          {a.answer === null ? <RulingForm a={a} /> : <Answered a={a} />}
        </div>
      );
    case 'policy_approval':
      return <PolicyAsk a={a} body={body} />;
    case 'changes':
      return <ChangesAsk a={a} body={body} />;
    default:
      return (
        <>
          {typeof body.holds === 'number' ? <p className="muted">{t('ask_holds', { id: body.holds })}</p> : null}
          <AskView a={a} />
        </>
      );
  }
}
