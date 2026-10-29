// An ask an architect put to the owner, as the owner reads and answers it, by its kind: a question, laid out as an
// engineer's ask is, with the matters it holds; an open choice, which holds no wake; the study of a new policy to read;
// and the policy to approve, with its probes and every reading of it by a new architect. Every kind but the question,
// which the card every ask shares heads, is a block under its number, kind and the cell and round that put it. While
// it waits, its form: an option clicked and words; or one of the choices its cell offers (satisfied, study on or no
// change; approved or not good enough) and words, needed unless the owner agrees. Each answer goes to the interface's
// answer of an ask, and the engine goes on.

import { useState } from 'react';

import type { JsonValue, OwnerAsk } from '../api';
import { choicesOf, rulingAnswer } from '../architect';
import { time } from '../format';
import { postJson } from '../http';
import { Field, OptionBoxes } from '../item/Shapes';
import { isObj, label, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, By, type OpenRound, RoundLink, Section, SendForm } from '../parts';
import { t, type Key } from '../text';
import { PolicyText, ProbesView, ReadingView, StudyView } from './ArchitectViews';
import { AnswerForm, AskView, decided } from './Decide';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

const KINDS: Record<string, Key> = {
  ask: 'ask_kind_ask',
  open: 'ask_kind_open',
  study: 'ask_kind_study',
  policy_approval: 'ask_kind_policy_approval',
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

/// The owner's answer once given: by whom and when, the choice, and the words.
function Answered({ a }: { a: OwnerAsk }) {
  const ans = isObj(a.answer) ? a.answer : {};
  const choice = str(ans.choice);
  return (
    <div className="decision-fact">
      <span className="decision-label">
        {t('ask_answer_by', { by: str(ans.by) })}
        {typeof ans.at === 'number' ? ` · ${time(ans.at)}` : ''}
      </span>
      {choice !== '' ? (
        <p className="prose">
          <strong>{label(choice)}</strong>
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
    <SendForm
      above={
        <>
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
        </>
      }
      words={{ text, set: setText, placeholder: t(agrees ? 'ruling_words_optional' : 'ruling_words_needed') }}
      label={t('answer_send')}
      ready={answer !== null}
      busy={busy}
      why={t('ruling_not_ready')}
      onSend={() => {
        if (answer !== null) send(answer);
      }}
      error={error}
    />
  );
}

/// The policy to approve: its text, its probes folded, and each reading of it by a new architect with the verdict.
function PolicyAsk({ a, body, onRound }: { a: OwnerAsk; body: Obj; onRound?: OpenRound }) {
  const readings = objs(body.readings);
  const probes = Array.isArray(body.probes) ? body.probes.length : 0;
  return (
    <div className="card-view">
      <PolicyText value={body.policy ?? null} />
      <Section title={t('policy_probes')} n={probes}>
        <ProbesView value={body.probes ?? null} />
      </Section>
      <Section title={t('policy_readings')} n={readings.length} open>
        {readings.map((r, i) => (
          <ReadingView
            key={i}
            by={typeof r.round === 'number' ? <By work={a.work} round={r.round} onRound={onRound} /> : null}
            reading={r.reading ?? null}
            verdict={r.verdict ?? null}
          />
        ))}
      </Section>
      {a.answer === null ? <RulingForm a={a} /> : <Answered a={a} />}
    </div>
  );
}

/// An ask of the architect's, by its kind; its rounds open by `onRound` where the page shows them, else on the wake's
/// page.
export function ArchitectAsk({ a, onRound }: { a: OwnerAsk; onRound?: OpenRound }) {
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
      return <PolicyAsk a={a} body={body} onRound={onRound} />;
    default:
      return (
        <>
          {Array.isArray(body.holds) && body.holds.length > 0 ? (
            <p className="muted">{t('ask_holds', { ids: body.holds.map((x) => str(x)).join(', ') })}</p>
          ) : null}
          <AskView a={a} round={<RoundLink work={a.work} round={a.round} onRound={onRound} />} />
        </>
      );
  }
}

/// An ask of the architect's wherever the page shows it: a question as the card every ask shares; every other kind as
/// a block under its number, its kind, whether it waits, and the cell and round that put it.
export function ArchitectAskBlock({ a, onRound }: { a: OwnerAsk; onRound?: OpenRound }) {
  const body = isObj(a.body) ? a.body : {};
  const kind = str(body.kind);
  if (kind !== 'open' && kind !== 'study' && kind !== 'policy_approval') return <ArchitectAsk a={a} onRound={onRound} />;
  const head = (
    <>
      <strong>{t('ask_head', { id: a.id })}</strong>
      <span className="badge">{askKindWords(a)}</span>
      <span className="muted">{a.answer === null ? t('ask_waits') : t('ask_answered')}</span>
    </>
  );
  return (
    <Block id={`ask-${a.id}`} head={head} by={<By cell={str(body.cell)} work={a.work} round={a.round} onRound={onRound} />}>
      <ArchitectAsk a={a} onRound={onRound} />
    </Block>
  );
}
