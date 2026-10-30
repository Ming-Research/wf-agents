// An ask an architect put to the owner, as the owner reads and answers it, by its kind: a question, laid out as an
// engineer's ask is, with the matters it holds; and an open choice, which holds no work of the architect, a block under
// its number, kind and the cell and round that put it. In the one-to-one it is the architect's bubble, with no box:
// while it waits it is pinned at the bottom, and the owner's words with it ticked reply to it (the interface's say to
// the architect with `reply_to`); replied to, it is back where it was asked, the reply in it. Elsewhere it says where it
// is answered.

import { Label } from '@primer/react';

import type { JsonValue, OwnerAsk } from '../api';
import { time } from '../format';
import { Field, OptionBoxes } from '../item/Shapes';
import { isObj, label, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, By, type OpenRound, RoundLink, SubHeading } from '../parts';
import { t, type Key } from '../text';
import { SEP } from '../voices';
import { AnswerPart, AskView, AskWaited } from './Decide';

const objs = (v: JsonValue | undefined): Obj[] => (Array.isArray(v) ? v.filter(isObj) : []);

const KINDS: Record<string, Key> = {
  ask: 'ask_kind_ask',
  question: 'ask_kind_ask',
  open: 'ask_kind_open',
};

/// What an ask of the architect's is, in words, for its head.
export function askKindWords(a: OwnerAsk): string {
  const kind = isObj(a.body) ? str(a.body.kind) : '';
  const key = KINDS[kind];
  return key !== undefined ? t(key) : words(kind);
}

/// The owner's answer once given: by whom and when, the choice, and the words.
function Answered({ a }: { a: OwnerAsk }) {
  const ans = isObj(a.answer) ? a.answer : {};
  const choice = str(ans.choice);
  return (
    <div className="field">
      <SubHeading>
        {t('ask_answer_by', { by: str(ans.by) })}
        {typeof ans.at === 'number' ? `${SEP}${time(ans.at)}` : ''}
      </SubHeading>
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
/// alone until the owner answers; it holds no work of the architect.
function OpenAsk({ a, body }: { a: OwnerAsk; body: Obj }) {
  const choice = isObj(body.choice) ? body.choice : {};
  const options = objs(choice.options);
  const affects = Array.isArray(body.affects) ? body.affects.map((x) => str(x)) : [];
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
      />
      {choice.confidence !== undefined ? (
        <Field label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
      ) : null}
      {affects.length > 0 ? (
        <div className="field">
          <SubHeading>{t('ask_affects')}</SubHeading>
          <ul className="arch-list">
            {affects.map((x, i) => (
              <li key={i}>
                <MarkdownInline text={x} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <AnswerPart a={a} given={<Answered a={a} />} />
    </div>
  );
}

/// An ask of the architect's, by its kind; its rounds open by `onRound` where the page shows them, else on the page of
/// the architect's work.
export function ArchitectAsk({ a, onRound }: { a: OwnerAsk; onRound?: OpenRound }) {
  const body = isObj(a.body) ? a.body : {};
  switch (str(body.kind)) {
    case 'question':
      return (
        <div className="card-view">
          <Markdown text={str(body.question)} />
          <AnswerPart a={a} given={<Answered a={a} />} />
        </div>
      );
    case 'open':
      return <OpenAsk a={a} body={body} />;
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

/// An ask of the architect's wherever the page shows it: a question as the card every ask shares; an open choice as a
/// block under its number, its kind, whether it waits, and the cell and round that put it.
export function ArchitectAskBlock({ a, onRound }: { a: OwnerAsk; onRound?: OpenRound }) {
  const body = isObj(a.body) ? a.body : {};
  if (str(body.kind) !== 'open') return <ArchitectAsk a={a} onRound={onRound} />;
  const head = (
    <>
      <strong>{t('ask_head', { id: a.id })}</strong>
      <Label variant="secondary">{askKindWords(a)}</Label>
      <span className="muted">{a.answer === null ? t('ask_waits') : t('ask_answered')}</span>
      <AskWaited a={a} />
    </>
  );
  return (
    <Block id={`ask-${a.id}`} head={head} by={<By cell={str(body.cell)} work={a.work} round={a.round} onRound={onRound} />}>
      <ArchitectAsk a={a} onRound={onRound} />
    </Block>
  );
}
