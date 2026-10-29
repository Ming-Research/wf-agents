// An ask an architect put to the owner, as the owner reads and answers it, by its kind: a question, laid out as an
// engineer's ask is, with the matters it holds; an open choice, which holds no wake; the study of a new policy to read;
// and the policy to approve, with its probes and every reading of it by a new architect. Every kind but the question,
// which the card every ask shares heads, is a block under its number, kind and the cell and round that put it. In the
// conversation it is in, it is the architect's bubble, answered in that bubble by a button (an option, or one of the
// choices its cell offers: satisfied, study on or no change; approved or not good enough) and the owner's words in a
// small box, needed unless the owner agrees; elsewhere it says where it is answered. Each answer goes to the
// interface's answer of an ask, and the engine goes on. The documents an ask is about (the study, the policy to approve
// and what changed in it since the owner last read it) are folded, each as one document.

import { Label } from '@primer/react';

import type { JsonValue, OwnerAsk } from '../api';
import { time } from '../format';
import { Field, OptionBoxes } from '../item/Shapes';
import { isObj, label, str, words, type Obj } from '../labelled';
import { Markdown, MarkdownInline } from '../markdown';
import { Block, By, type OpenRound, RoundLink, Section, SubHeading } from '../parts';
import { t, type Key } from '../text';
import { PolicyText, ProbesView, ReadingView, StudyView } from './ArchitectViews';
import { AnswerPart, AskView } from './Decide';

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

/// The owner's answer once given: by whom and when, the choice, and the words.
function Answered({ a }: { a: OwnerAsk }) {
  const ans = isObj(a.answer) ? a.answer : {};
  const choice = str(ans.choice);
  return (
    <div className="field">
      <SubHeading>
        {t('ask_answer_by', { by: str(ans.by) })}
        {typeof ans.at === 'number' ? ` · ${time(ans.at)}` : ''}
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
/// alone until the owner answers; it holds no wake.
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

/// One correction of the text of a policy being set, as the round that made it handed it in: why, and each edit.
function Revision({ r }: { r: Obj }) {
  const edits = objs(r.edits);
  const edit = (e: Obj): string => {
    switch (str(e.kind)) {
      case 'change':
        return t('revision_change', { old: str(e.old), new: str(e.new) });
      case 'add_principle':
        return t('revision_add_principle', { text: str(e.text) });
      case 'add_signal':
        return t('revision_add_signal', { what: str(e.what), how: str(e.how) });
      case 'remove':
        return t('revision_remove', { part: words(str(e.part)), no: str(e.no) });
      default:
        return JSON.stringify(e);
    }
  };
  return (
    <div className="agent-box">
      {str(r.why) !== '' ? <Markdown text={str(r.why)} /> : null}
      <ol className="document-list">
        {edits.map((e, i) => (
          <li key={i}>
            <MarkdownInline text={edit(e)} />
          </li>
        ))}
      </ol>
    </div>
  );
}

/// The policy to approve: its text, folded open while it waits, as one document; the corrections of the text since
/// the owner last read it; its probes folded; and each reading of it by a new architect with the verdict.
function PolicyAsk({ a, body, onRound, revisions }: {
  a: OwnerAsk;
  body: Obj;
  onRound?: OpenRound;
  revisions: Obj[];
}) {
  const readings = objs(body.readings);
  const probes = Array.isArray(body.probes) ? body.probes.length : 0;
  return (
    <div className="card-view">
      <details className="talk-fold" open={a.answer === null}>
        <summary>{t('ask_policy_fold')}</summary>
        <div className="talk-fold-body">
          <PolicyText value={body.policy ?? null} />
        </div>
      </details>
      {revisions.length > 0 ? (
        <details className="talk-fold">
          <summary>{t('ask_policy_changed', { n: revisions.length })}</summary>
          <div className="talk-fold-body">
            {revisions.map((r, i) => (
              <Revision key={i} r={r} />
            ))}
          </div>
        </details>
      ) : null}
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
      <AnswerPart a={a} given={<Answered a={a} />} />
    </div>
  );
}

/// An ask of the architect's, by its kind; its rounds open by `onRound` where the page shows them, else on the wake's
/// page; for the policy to approve, the corrections of its text since the owner last read it (`revisions`).
export function ArchitectAsk({ a, onRound, revisions = null }: { a: OwnerAsk; onRound?: OpenRound; revisions?: JsonValue }) {
  const body = isObj(a.body) ? a.body : {};
  switch (str(body.kind)) {
    case 'open':
      return <OpenAsk a={a} body={body} />;
    case 'study':
      return (
        <div className="card-view">
          <details className="talk-fold" open={a.answer === null}>
            <summary>{t('ask_study_fold')}</summary>
            <div className="talk-fold-body">
              <StudyView value={body.study ?? null} />
            </div>
          </details>
          <AnswerPart a={a} given={<Answered a={a} />} />
        </div>
      );
    case 'policy_approval':
      return <PolicyAsk a={a} body={body} onRound={onRound} revisions={objs(revisions ?? undefined)} />;
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
export function ArchitectAskBlock({ a, onRound, revisions = null }: {
  a: OwnerAsk;
  onRound?: OpenRound;
  revisions?: JsonValue;
}) {
  const body = isObj(a.body) ? a.body : {};
  const kind = str(body.kind);
  if (kind !== 'open' && kind !== 'study' && kind !== 'policy_approval') return <ArchitectAsk a={a} onRound={onRound} />;
  const head = (
    <>
      <strong>{t('ask_head', { id: a.id })}</strong>
      <Label variant="secondary">{askKindWords(a)}</Label>
      <span className="muted">{a.answer === null ? t('ask_waits') : t('ask_answered')}</span>
    </>
  );
  return (
    <Block id={`ask-${a.id}`} head={head} by={<By cell={str(body.cell)} work={a.work} round={a.round} onRound={onRound} />}>
      <ArchitectAsk a={a} onRound={onRound} revisions={revisions} />
    </Block>
  );
}
