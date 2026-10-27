// What the owner decides on the page. A design card says first how many decisions it holds, the node they share, and
// that it asks nothing now but is confirmed before the merge; then each decision is a numbered fold under a one-line
// title, with its kind in words that say it is the agent's view and how sure the agent is: open, the decision and why;
// folded, the approaches compared and what would overturn it. While it waits for a ruling, the owner rules on each
// decision (confirm, disagree with a reason, or click another of its approaches to choose it) and sends the verdicts on
// all of them at once, after a summary of what is sent. A replaced design card says only what replaced it, when, in
// which round and at which cell, and what changed, its full text folded. An ask of the owner is laid out as what it
// asks about, each point under its kind coloured by the kind's group, and for a decision the options with the one
// recommended; while it waits, the owner clicks an option and says what more there is to say. Options and approaches
// are boxes one under another, each its name over what it does and why. Both write what the command line writes, and
// the engine goes on at once.

import { useState, type MouseEvent } from 'react';

import type { AskKind, DesignCard, JsonValue, OwnerAsk } from '../api';
import { roundName, time } from '../format';
import { postJson } from '../http';
import { OptionBoxes } from '../item/Shapes';
import { isObj, Labelled, str, words, type Obj } from '../labelled';
import { reread } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { Link, navigate, workHref } from '../route';
import { t } from '../text';

export { words };

/// What the owner decided has been written: everything it shows is read again.
function decided() {
  reread('works', 'work:*', 'items', 'item:*', 'round:*');
}

/// Brings an element of the page into view, opening whatever folds it; false when the page does not have it.
export function bringIntoView(id: string): boolean {
  const el = document.getElementById(id);
  if (el === null) return false;
  for (let p: HTMLElement | null = el; p !== null; p = p.parentElement) {
    if (p instanceof HTMLDetailsElement) p.open = true;
  }
  el.scrollIntoView({ block: 'start' });
  return true;
}

function Text({ label, value }: { label: string; value: JsonValue | undefined }) {
  const s = str(value);
  if (s === '') return null;
  return (
    <div className="decision-fact">
      <span className="decision-label">{label}</span>
      <Markdown text={s} />
    </div>
  );
}

/// The first sentence of a text, as a title on one line: up to its first full stop, at most 120 characters.
export function firstSentence(text: string): string {
  const line = text.trim().split('\n')[0] ?? '';
  // A full stop, question or exclamation mark ends a sentence before a space or the end; those of Chinese and Japanese
  // end it on their own.
  const m = /^(.+?(?:[.!?](?=\s|$)|[。！？]))/.exec(line);
  const one = m !== null ? m[1] : line;
  return one.length > 120 ? `${one.slice(0, 119)}…` : one;
}

/// How firm the agent holds a decision, in words that say it is the agent's view and not a ruling.
function kindWords(d: Obj): string {
  switch (str(d.settled)) {
    case 'settled':
      return t('kind_settled');
    case 'provisional':
      return t('kind_provisional', { what: firstSentence(str(d.uncertain)) });
    case 'to_check':
      return t('kind_to_check', { what: firstSentence(str(d.assume)) });
  }
  return words(str(d.settled));
}

/// What the owner set for one decision before sending: a verdict, and for a disagreement why and any approach chosen
/// instead.
export type Verdict = { verdict: 'confirm' | 'disagree' | null; text: string; option: string };

const UNSET: Verdict = { verdict: null, text: '', option: '' };

/// Whether a decision's verdict can be sent: a confirmation, or a disagreement that says why or chooses an approach.
export function verdictReady(v: Verdict): boolean {
  return v.verdict === 'confirm' || (v.verdict === 'disagree' && (v.text.trim() !== '' || v.option !== ''));
}

/// One decision's verdict in words, as the summary before sending and the rulings given say it.
export function verdictLine(n: number, verdict: string | null, option: string, text: string): string {
  const head =
    verdict === 'confirm'
      ? t('rule_line_confirm', { n })
      : verdict === 'disagree'
        ? option !== ''
          ? t('rule_line_choose', { n, option })
          : t('rule_line_disagree', { n })
        : t('rule_line_unset', { n });
  return verdict === 'disagree' && text.trim() !== '' ? `${head}: ${text.trim()}` : head;
}

/// What a click on an approach of a decision sets: the approach recommended confirms the decision; another chooses it
/// instead, disagreeing; the approach chosen already, clicked again, is cleared.
export function pickApproach(v: Verdict, name: string, recommended: string): Verdict {
  if (name === v.option) return { ...v, option: '', verdict: v.text.trim() !== '' ? 'disagree' : null };
  if (name === recommended) return { verdict: 'confirm', text: '', option: '' };
  return { ...v, option: name, verdict: 'disagree' };
}

/// The owner's verdict on one decision while the card waits: confirm, or disagree saying why; another approach is
/// chosen by clicking it among the approaches compared.
function RuleDecision({ v, set }: { v: Verdict; set: (v: Verdict) => void }) {
  return (
    <div className="decide decision-rule">
      <div className="decide-row">
        <button
          type="button"
          className={`decide-button${v.verdict === 'confirm' ? ' on' : ''}`}
          onClick={() => set({ verdict: 'confirm', text: '', option: '' })}
          aria-pressed={v.verdict === 'confirm'}
        >
          {t('rule_confirm')}
        </button>
        <button
          type="button"
          className={`decide-button${v.verdict === 'disagree' ? ' on' : ''}`}
          onClick={() => set({ ...v, verdict: 'disagree' })}
          aria-pressed={v.verdict === 'disagree'}
        >
          {t('rule_disagree')}
        </button>
      </div>
      {v.verdict === 'disagree' ? (
        <>
          {v.option !== '' ? <p className="muted pick-hint">{t('rule_chosen_option', { option: v.option })}</p> : null}
          <textarea
            value={v.text}
            rows={2}
            placeholder={t(v.option !== '' ? 'rule_why_more_hint' : 'rule_why_hint')}
            onChange={(e) => set({ ...v, text: e.target.value })}
          />
        </>
      ) : (
        <p className="muted pick-hint">{t('rule_pick_hint')}</p>
      )}
    </div>
  );
}

/// One decision of a design card, a numbered fold under a one-line title, with whether it is new, how firm the agent
/// holds it and how sure it is: open, the decision and why (and what it was, when it changes one); folded, the
/// approaches compared, which the owner may click to choose one for this decision, how sure and what would overturn it.
function Decision(props: { d: Obj; n: number; showNode: boolean; v: Verdict | null; set: ((v: Verdict) => void) | null }) {
  const { d, n, v, set } = props;
  const change = str(d.change);
  const options = Array.isArray(d.options) ? d.options.filter(isObj) : [];
  const chosen = str(d.chosen);
  const sure = str(d.sure);
  const decision = change === 'remove' ? d.before : d.after;
  const pick = set !== null && v !== null ? (name: string) => set(pickApproach(v, name, chosen)) : null;
  return (
    <details className="decision" open>
      <summary className="decision-head">
        <span className="decision-n">{n}.</span>
        <span className="decision-title">
          <MarkdownInline text={firstSentence(str(decision))} />
        </span>
        {change === 'add' ? <span className="badge">{t('decision_new')}</span> : null}
        {change === 'remove' ? <span className="badge">{t('decision_remove')}</span> : null}
        <span className={`decision-kind decision-${str(d.settled)}`}>{kindWords(d)}</span>
        {sure !== '' ? <span className="muted">{t('decision_sure_short', { sure })}</span> : null}
      </summary>
      <div className="decision-body">
        {props.showNode ? <code>{str(d.node)}</code> : null}
        <Text label={t('decision_is')} value={decision} />
        {change === 'change' ? <Text label={t('decision_before')} value={d.before} /> : null}
        <Text label={t('decision_why')} value={d.why} />
        <details className="decision-more">
          <summary className="muted">{t('decision_more', { n: options.length })}</summary>
          <div className="decision-more-body">
            {options.length > 0 ? (
              <OptionBoxes
                options={options}
                fields={[
                  ['does', t('option_does')],
                  ['verdict', t('option_verdict')],
                ]}
                recommended={chosen}
                mark={t('decision_chosen')}
                picked={v?.option ?? ''}
                onPick={pick}
              />
            ) : null}
            {sure !== '' ? (
              <Text label={t('decision_sure', { sure })} value={`${t(`sure_${sure}` as 'sure_1')}. ${str(d.sure_why)}`} />
            ) : null}
            <Text label={t('decision_overturn')} value={d.overturn} />
            <Text label={t('decision_uncertain')} value={d.uncertain} />
            <Text label={t('decision_reopen')} value={d.reopen} />
            <Text label={t('decision_assume')} value={d.assume} />
            <Text label={t('decision_check')} value={d.check} />
            <Text label={t('decision_carried_by')} value={d.carried_by} />
            <Text label={t('decision_from_parent')} value={d.from_parent} />
          </div>
        </details>
        {set !== null && v !== null ? <RuleDecision v={v} set={set} /> : null}
      </div>
    </details>
  );
}

/// The rulings given, in order: a verdict on each decision, or for one made before rulings were per decision, one on
/// the card.
function Rulings({ rulings }: { rulings: JsonValue }) {
  const list = Array.isArray(rulings) ? rulings.filter(isObj) : [];
  if (list.length === 0) return null;
  return (
    <ul className="rulings">
      {list.map((r, i) => {
        const each = Array.isArray(r.decisions) ? r.decisions.filter(isObj) : null;
        return (
          <li key={i}>
            <strong>
              {each !== null ? t('ruling_of', { by: str(r.by) }) : t('ruling_by', { by: str(r.by), verdict: str(r.verdict) })}
            </strong>
            {each === null && r.option ? <span> · {t('ruling_option', { option: str(r.option) })}</span> : null}
            {typeof r.at === 'number' ? <span className="muted"> · {time(r.at)}</span> : null}
            {each !== null ? (
              <ul>
                {each.map((d, j) => (
                  <li key={j}>
                    {verdictLine(Number(d.decision), str(d.verdict), str(d.option), str(d.text))}
                  </li>
                ))}
              </ul>
            ) : str(r.text) !== '' ? (
              <Markdown text={str(r.text)} />
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

/// What the owner sends: a summary of the verdict set on each decision, and the button that sends them all at once,
/// once every decision has one that can be sent.
function RuleForm({ c, verdicts }: { c: DesignCard; verdicts: Verdict[] }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ready = verdicts.every(verdictReady);
  const send = () => {
    setBusy(true);
    setError(null);
    const decisions = verdicts.map((v, i) => ({
      decision: i + 1,
      verdict: v.verdict ?? 'confirm',
      ...(v.verdict === 'disagree' && v.text.trim() !== '' ? { text: v.text.trim() } : {}),
      ...(v.verdict === 'disagree' && v.option !== '' ? { option: v.option } : {}),
    }));
    postJson(`/api/v1/design-cards/${c.id}/rule`, { decisions }).then(decided, (e: unknown) => {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return (
    <div className="decide rule-summary">
      <span className="decision-label">{t('rule_summary_title')}</span>
      <ul>
        {verdicts.map((v, i) => (
          <li key={i} className={verdictReady(v) ? undefined : 'muted'}>
            {verdictLine(i + 1, v.verdict, v.option, v.text)}
          </li>
        ))}
      </ul>
      <div className="decide-row">
        <button
          type="button"
          className="decide-button"
          onClick={send}
          disabled={busy || !ready}
          title={ready ? undefined : t('rule_not_ready')}
        >
          {t('rule_send')}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// A design card: how many decisions, the node they share, that it asks nothing now and is confirmed before the merge;
/// each decision; the rulings given; and while it waits, the owner's verdict on each decision and what is sent.
function CardBody({ c }: { c: DesignCard }) {
  const decisions = Array.isArray(c.body) ? c.body.filter(isObj) : isObj(c.body) ? [c.body] : [];
  const open = c.status === 'open';
  const [set, setSet] = useState<Verdict[]>([]);
  const verdicts = decisions.map((_, i) => set[i] ?? UNSET);
  const setOne = (i: number) => (v: Verdict) =>
    setSet(() => verdicts.map((x, j) => (j === i ? v : x)));
  const nodes = [...new Set(decisions.map((d) => str(d.node)))];
  const shared = nodes.length === 1 && nodes[0] !== '' ? nodes[0] : null;
  if (decisions.length === 0) {
    return (
      <div className="card-view">
        <Labelled value={c.body} />
        <Rulings rulings={c.rulings} />
      </div>
    );
  }
  return (
    <div className="card-view">
      <div className="card-head">
        <strong>{decisions.length === 1 ? t('card_one_decision') : t('card_decisions', { n: decisions.length })}</strong>
        {shared !== null ? (
          <>
            {' '}
            <code>{shared}</code>
          </>
        ) : null}
        <p className="muted">{t('card_asks_nothing')}</p>
      </div>
      {decisions.map((d, i) => (
        <Decision
          key={i}
          d={d}
          n={i + 1}
          showNode={shared === null}
          v={open ? verdicts[i] : null}
          set={open ? setOne(i) : null}
        />
      ))}
      <Rulings rulings={c.rulings} />
      {open ? <RuleForm c={c} verdicts={verdicts} /> : null}
    </div>
  );
}

/// A link to a design card: brought into view where the page shows it, else on its item's page.
function CardLink({ id, item }: { id: number; item: number | null }) {
  const to = item !== null ? `/items/${item}#design-card-${id}` : null;
  const go = (e: MouseEvent<HTMLAnchorElement>) => {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    e.stopPropagation();
    if (!bringIntoView(`design-card-${id}`) && to !== null) navigate(to);
  };
  return (
    <a href={to ?? `#design-card-${id}`} onClick={go}>
      {t('card_name', { id })}
    </a>
  );
}

/// A replaced design card: what replaced it, when, in which round and at which cell, and what the newer one changed
/// and why when its agent said; the card as it was, folded.
function Replaced({ c }: { c: DesignCard }) {
  const r = c.replaced;
  const cell = r === null || r.cell === null ? '' : r.title !== null ? `${r.cell} (${r.title})` : r.cell;
  return (
    <div className="card-view">
      {r === null ? (
        <p className="card-replaced muted">{t('card_replaced_none')}</p>
      ) : (
        <p className="card-replaced">
          {t('card_replaced_by')} <CardLink id={r.design_card} item={c.item} />
          {r.at !== null ? <span className="muted"> · {time(r.at)}</span> : null} ·{' '}
          <Link to={workHref(r.work, { round: r.round })}>
            {t('card_replaced_in', { round: roundName(r.work, r.round), cell })}
          </Link>
        </p>
      )}
      {r?.changed ? <Markdown text={r.changed} /> : null}
      <details className="card-was">
        <summary className="muted">{t('card_as_it_was')}</summary>
        <CardBody c={c} />
      </details>
    </div>
  );
}

/// A design card as the owner reads and rules on it.
export function DesignCardView({ c }: { c: DesignCard }) {
  return c.status === 'replaced' ? <Replaced c={c} /> : <CardBody c={c} />;
}

/// The kind of a point an ask is about, as a chip coloured by the kind's group, so that the sort of question shows at
/// a glance.
export function KindChip({ k }: { k: AskKind }) {
  return (
    <span
      className={`badge kind kind-${k.group ?? 'none'}`}
      title={k.group !== null ? t('kind_group', { group: words(k.group) }) : undefined}
    >
      {words(k.kind)}
    </span>
  );
}

/// The owner's answer to an ask that waits: the option clicked in its choice, when one is, and words.
function AnswerForm({ a, picked, options }: { a: OwnerAsk; picked: string; options: boolean }) {
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const answer =
    picked !== '' ? `${t('answer_chosen', { option: picked })}${text.trim() !== '' ? ` ${text.trim()}` : ''}` : text.trim();
  const send = () => {
    setBusy(true);
    setError(null);
    postJson(`/api/v1/asks/${a.id}/answer`, { text: answer }).then(decided, (e: unknown) => {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return (
    <div className="decide">
      {options ? (
        <p className="muted pick-hint">
          {picked === '' ? t('answer_pick_hint') : t('answer_picked', { option: picked })}
        </p>
      ) : null}
      <textarea
        value={text}
        rows={3}
        placeholder={t(options ? 'answer_more_hint' : 'answer_text_hint')}
        onChange={(e) => setText(e.target.value)}
        disabled={busy}
      />
      <div className="decide-row">
        <button type="button" className="decide-button" onClick={send} disabled={busy || answer === ''}>
          {t('answer_send')}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// A text an agent wrote, as it reads.
export function Written({ text }: { text: string }) {
  return <Markdown text={text} />;
}

/// What an ask holds: what the cells handed in, each with its points and its choice. The engine keeps them under
/// `asked`, each with what it is and its value; an ask written as one is its own.
function parts(body: Obj): Obj[] {
  const asked = Array.isArray(body.asked)
    ? body.asked
        .filter(isObj)
        .map((x) => x.value)
        .filter((v): v is Obj => v !== undefined && isObj(v))
    : [];
  return asked.length > 0 ? asked : [body];
}

/// An ask of the owner as the owner reads and answers it: for each point, its kind as a chip, what it says (its code
/// kept as code) and what was tried; the choice, when there is one, with the option recommended, which the owner
/// answers by clicking an option; then the answer, or the form to give it.
export function AskView({ a }: { a: OwnerAsk }) {
  const [picked, setPicked] = useState('');
  const body = isObj(a.body) ? a.body : {};
  const held = parts(body);
  const points = held.flatMap((x) => (Array.isArray(x.points) ? x.points.filter(isObj) : []));
  const choice = held.map((x) => x.choice).find((c): c is Obj => c !== undefined && isObj(c)) ?? null;
  const options = choice !== null && Array.isArray(choice.options) ? choice.options.filter(isObj) : [];
  const answer = a.answer !== null && isObj(a.answer) ? a.answer : null;
  const kind = (k: string): AskKind => a.kinds.find((x) => x.kind === k) ?? { kind: k, group: null };
  const waits = answer === null;
  return (
    <div className="card-view">
      {body.to === 'architect' ? <p className="muted">{t('ask_for_architect')}</p> : null}
      {points.map((p, i) => (
        <div key={i} className="decision-fact">
          <div>
            <KindChip k={kind(str(p.kind))} />
          </div>
          <Markdown text={str(p.what)} />
          {str(p.tried) !== '' ? (
            <div className="ask-tried">
              <span className="decision-label">{t('ask_tried_label')}</span>
              <Markdown text={str(p.tried)} />
            </div>
          ) : null}
        </div>
      ))}
      {choice !== null ? (
        <>
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
            <Text label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
          ) : null}
        </>
      ) : null}
      {points.length === 0 && choice === null ? <Labelled value={a.body} /> : null}
      {answer !== null ? (
        <div className="decision-fact">
          <span className="decision-label">
            {t('ask_answer_by', { by: str(answer.by) })}
            {typeof answer.at === 'number' ? ` · ${time(answer.at)}` : ''}
          </span>
          <Markdown text={str(answer.text)} />
        </div>
      ) : (
        <AnswerForm a={a} picked={picked} options={options.length > 0} />
      )}
    </div>
  );
}
