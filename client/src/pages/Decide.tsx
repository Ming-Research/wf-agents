// What the owner decides on the page. A design card is laid out as the decisions it holds (for each: whether it is
// settled, the node, what changes from what to what and why, the approaches compared with their verdicts, how sure and
// what would overturn it), with its rulings, and while it waits for a ruling, the owner confirms it or disagrees with a
// reason, and may click another approach to rule for it. A replaced design card says only what replaced it, when, in
// which round and at which cell, its full text folded. An ask of the owner is laid out as what it asks about, each
// point under its kind coloured by the kind's group, and for a decision the options with the one recommended; while it
// waits, the owner clicks an option and says what more there is to say. Both write what the command line writes, and
// the engine goes on at once.

import { useState, type MouseEvent } from 'react';

import type { AskKind, DesignCard, JsonValue, OwnerAsk } from '../api';
import { roundName, time } from '../format';
import { postJson } from '../http';
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

/// The options of a choice, which are also how the owner chooses: a click picks an option and a second click clears
/// it. The option the agent recommends and the one the owner picked are marked apart, and may be the same.
function Options(props: {
  options: Obj[];
  /// The fields shown beside each option's name: what it does, then the verdict or the cost.
  fields: [string, string];
  recommended: string;
  /// What the recommended option is called here.
  mark: string;
  picked: string;
  onPick: ((name: string) => void) | null;
}) {
  const pick = props.onPick;
  return (
    <table className="list decision-options">
      <tbody>
        {props.options.map((o, i) => {
          const name = str(o.name);
          const recommended = name !== '' && name === props.recommended;
          const mine = name !== '' && name === props.picked;
          const cls = [recommended ? 'recommended' : '', mine ? 'picked' : '', pick !== null ? 'pickable' : '']
            .filter((c) => c !== '')
            .join(' ');
          return (
            <tr
              key={i}
              className={cls === '' ? undefined : cls}
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
              <td>
                <strong>
                  <MarkdownInline text={name} />
                </strong>
                {recommended ? <div className="option-recommended">{props.mark}</div> : null}
                {mine ? <div className="option-picked">{t('option_yours')}</div> : null}
              </td>
              <td>
                <Markdown text={str(o[props.fields[0]])} />
              </td>
              <td className="muted">
                <Markdown text={str(o[props.fields[1]])} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/// One decision of a design card.
function Decision({ d, picked, onPick }: { d: Obj; picked: string; onPick: ((name: string) => void) | null }) {
  const settled = str(d.settled);
  const options = Array.isArray(d.options) ? d.options.filter(isObj) : [];
  const sure = str(d.sure);
  return (
    <div className="decision">
      <div className="decision-head">
        <span className={`badge decision-${settled}`}>{words(settled)}</span>
        <span className="badge">{t(`decision_${str(d.change)}` as 'decision_add')}</span>
        <code>{str(d.node)}</code>
      </div>
      <Text label={t('decision_before')} value={d.before} />
      <Text label={t('decision_after')} value={d.after} />
      <Text label={t('decision_why')} value={d.why} />
      {options.length > 0 ? (
        <Options
          options={options}
          fields={['does', 'verdict']}
          recommended={str(d.chosen)}
          mark={t('decision_chosen')}
          picked={picked}
          onPick={onPick}
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
  );
}

/// The rulings given, in order.
function Rulings({ rulings }: { rulings: JsonValue }) {
  const list = Array.isArray(rulings) ? rulings.filter(isObj) : [];
  if (list.length === 0) return null;
  return (
    <ul className="rulings">
      {list.map((r, i) => (
        <li key={i}>
          <strong>{t('ruling_by', { by: str(r.by), verdict: str(r.verdict) })}</strong>
          {r.option ? <span> · {t('ruling_option', { option: str(r.option) })}</span> : null}
          {typeof r.at === 'number' ? <span className="muted"> · {time(r.at)}</span> : null}
          {str(r.text) !== '' ? <Markdown text={str(r.text)} /> : null}
        </li>
      ))}
    </ul>
  );
}

/// The owner's ruling on a design card that waits for one: confirm, or disagree with a reason; the approach clicked
/// in the card, when one is, is ruled for instead of the one recommended.
function RuleForm(props: { c: DesignCard; option: string; options: boolean; recommended: boolean }) {
  const { c, option, options } = props;
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const send = (verdict: 'confirm' | 'disagree') => {
    setBusy(true);
    setError(null);
    const body = option === '' ? { verdict, text } : { verdict, text, option };
    postJson(`/api/v1/design-cards/${c.id}/rule`, body).then(decided, (e: unknown) => {
      setBusy(false);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return (
    <div className="decide">
      {options ? (
        <p className="muted pick-hint">
          {option === ''
            ? t('rule_pick_hint')
            : t(props.recommended ? 'rule_picked_recommended' : 'rule_picked', { option })}
        </p>
      ) : null}
      <textarea
        value={text}
        rows={3}
        placeholder={t('rule_text_hint')}
        onChange={(e) => setText(e.target.value)}
        disabled={busy}
      />
      <div className="decide-row">
        <button type="button" className="decide-button" onClick={() => send('confirm')} disabled={busy}>
          {t('rule_confirm')}
        </button>
        <button
          type="button"
          className="decide-button"
          onClick={() => send('disagree')}
          disabled={busy || text.trim() === ''}
          title={text.trim() === '' ? t('rule_disagree_needs') : undefined}
        >
          {t('rule_disagree')}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// A design card's decisions and rulings, and while it waits, the ruling: one approach may be clicked across the
/// card, as a ruling chooses one.
function CardBody({ c }: { c: DesignCard }) {
  const [picked, setPicked] = useState('');
  const open = c.status === 'open';
  const decisions = Array.isArray(c.body) ? c.body.filter(isObj) : isObj(c.body) ? [c.body] : [];
  const options = decisions.some((d) => Array.isArray(d.options) && d.options.length > 0);
  const pick = open ? (name: string) => setPicked((p) => (p === name ? '' : name)) : null;
  return (
    <div className="card-view">
      {decisions.length === 0 ? (
        <Labelled value={c.body} />
      ) : (
        decisions.map((d, i) => <Decision key={i} d={d} picked={open ? picked : ''} onPick={pick} />)
      )}
      <Rulings rulings={c.rulings} />
      {open ? (
        <RuleForm
          c={c}
          option={picked}
          options={options}
          recommended={decisions.some((d) => str(d.chosen) === picked)}
        />
      ) : null}
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
          <Options
            options={options}
            fields={['does', 'cost_risk']}
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
