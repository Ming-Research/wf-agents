// What the owner decides on the page. A design card is laid out as the decisions it holds (for each: whether it is
// settled, the node, what changes from what to what and why, the approaches compared with their verdicts, how sure and
// what would overturn it), with its rulings, and while it waits for a ruling, the owner confirms it or disagrees with a
// reason. An ask of the owner is laid out as what it asks about and, for a decision, the options with the one
// recommended, and while it waits, the owner picks an option and says what more there is to say. Both write what the
// command line writes, and the engine goes on at once.

import { useState } from 'react';

import type { DesignCard, JsonValue, OwnerAsk } from '../api';
import { Json } from '../components';
import { time } from '../format';
import { postJson } from '../http';
import { reread } from '../live';
import { t } from '../text';

type Obj = { [key in string]: JsonValue };

const isObj = (v: JsonValue | undefined): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);

const str = (v: JsonValue | undefined): string =>
  typeof v === 'string' ? v : v === undefined || v === null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v);

/// A name as words: `to_check` is "to check".
export const words = (s: string | null | undefined) => (s ?? '').replaceAll('_', ' ');

/// What the owner decided has been written: everything it shows is read again.
function decided() {
  reread('works', 'work:*', 'items', 'item:*', 'round:*');
}

function Text({ label, value }: { label: string; value: JsonValue | undefined }) {
  const s = str(value);
  if (s === '') return null;
  return (
    <div className="decision-fact">
      <span className="decision-label">{label}</span>
      <p className="prose">{s}</p>
    </div>
  );
}

/// One decision of a design card.
function Decision({ d }: { d: Obj }) {
  const settled = str(d.settled);
  const options = Array.isArray(d.options) ? d.options.filter(isObj) : [];
  const chosen = str(d.chosen);
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
        <table className="list decision-options">
          <tbody>
            {options.map((o, i) => {
              const name = str(o.name);
              return (
                <tr key={i} className={name === chosen ? 'chosen' : undefined}>
                  <td>
                    <strong>{name}</strong>
                    {name === chosen ? <div className="good">{t('decision_chosen')}</div> : null}
                  </td>
                  <td className="prose">{str(o.does)}</td>
                  <td className="prose muted">{str(o.verdict)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
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
          {str(r.text) !== '' ? <p className="prose">{str(r.text)}</p> : null}
        </li>
      ))}
    </ul>
  );
}

/// The owner's ruling on a design card that waits for one: confirm, or disagree with a reason; another approach may be
/// chosen instead of the one recommended.
function RuleForm({ c, options }: { c: DesignCard; options: string[] }) {
  const [text, setText] = useState('');
  const [option, setOption] = useState('');
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
      <textarea
        value={text}
        rows={3}
        placeholder={t('rule_text_hint')}
        onChange={(e) => setText(e.target.value)}
        disabled={busy}
      />
      {options.length > 0 ? (
        <label className="decide-row">
          {t('rule_option')}{' '}
          <select value={option} onChange={(e) => setOption(e.target.value)} disabled={busy}>
            <option value="">{t('rule_option_none')}</option>
            {options.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </select>
        </label>
      ) : null}
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

/// A design card as the owner reads and rules on it.
export function DesignCardView({ c }: { c: DesignCard }) {
  const decisions = Array.isArray(c.body) ? c.body.filter(isObj) : isObj(c.body) ? [c.body] : [];
  const options = [
    ...new Set(
      decisions.flatMap((d) => (Array.isArray(d.options) ? d.options.filter(isObj).map((o) => str(o.name)) : [])),
    ),
  ].filter((o) => o !== '');
  return (
    <div className="card-view">
      {decisions.length === 0 ? <Json value={c.body} /> : decisions.map((d, i) => <Decision key={i} d={d} />)}
      <Rulings rulings={c.rulings} />
      {c.status === 'open' ? <RuleForm c={c} options={options} /> : null}
    </div>
  );
}

/// The owner's answer to an ask that waits: an option of its choice, when it has one, and words.
function AnswerForm({ a, options }: { a: OwnerAsk; options: string[] }) {
  const [picked, setPicked] = useState('');
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
      {options.length > 0 ? (
        <div className="decide-options">
          {options.map((o) => (
            <label key={o} className="decide-row">
              <input type="radio" checked={picked === o} onChange={() => setPicked(o)} disabled={busy} /> {o}
            </label>
          ))}
        </div>
      ) : null}
      <textarea
        value={text}
        rows={3}
        placeholder={t(options.length > 0 ? 'answer_more_hint' : 'answer_text_hint')}
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

/// A text an agent wrote cut into its paragraphs, each prose or code: code is a block between ``` fences, or a
/// paragraph of three lines or more none of which ends a sentence, such as a command to run.
export function blocks(text: string): Array<{ code: boolean; text: string }> {
  const out: Array<{ code: boolean; text: string }> = [];
  text.split(/^```[^\n]*$/m).forEach((segment, i) => {
    if (i % 2 === 1) {
      const code = segment.replace(/^\n+|\n+$/g, '');
      if (code !== '') out.push({ code: true, text: code });
      return;
    }
    for (const para of segment.split(/\n[ \t]*\n/)) {
      const p = para.replace(/^\n+|\n+$/g, '');
      if (p.trim() === '') continue;
      const lines = p.split('\n');
      // A sentence ends with a full stop, an exclamation or a question mark, those of Chinese and Japanese too.
      const ends = (l: string) => /[\u3002\uff01\uff1f.!?]\s*$/.test(l);
      out.push({ code: lines.length >= 3 && !lines.some(ends), text: p });
    }
  });
  return out;
}

/// A text an agent wrote, as it reads: prose paragraphs as prose, code kept as code.
export function Written({ text }: { text: string }) {
  return (
    <>
      {blocks(text).map((b, i) =>
        b.code ? (
          <pre key={i} className="code-block">
            {b.text}
          </pre>
        ) : (
          <p key={i} className="prose">
            {b.text}
          </p>
        ),
      )}
    </>
  );
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

/// An ask of the owner as the owner reads and answers it: for each point, its kind in words, what it says (its code
/// kept as code) and what was tried; the choice, when there is one, with the option recommended; then the answer, or
/// the form to give it.
export function AskView({ a }: { a: OwnerAsk }) {
  const body = isObj(a.body) ? a.body : {};
  const held = parts(body);
  const points = held.flatMap((x) => (Array.isArray(x.points) ? x.points.filter(isObj) : []));
  const choice = held.map((x) => x.choice).find((c): c is Obj => c !== undefined && isObj(c)) ?? null;
  const options = choice !== null && Array.isArray(choice.options) ? choice.options.filter(isObj) : [];
  const recommended = choice !== null ? str(choice.recommended) : '';
  const answer = a.answer !== null && isObj(a.answer) ? a.answer : null;
  return (
    <div className="card-view">
      {body.to === 'architect' ? <p className="muted">{t('ask_for_architect')}</p> : null}
      {points.map((p, i) => (
        <div key={i} className="decision-fact">
          <span className="decision-label">{words(str(p.kind))}</span>
          <Written text={str(p.what)} />
          {str(p.tried) !== '' ? (
            <div className="ask-tried">
              <span className="decision-label">{t('ask_tried_label')}</span>
              <Written text={str(p.tried)} />
            </div>
          ) : null}
        </div>
      ))}
      {choice !== null ? (
        <>
          <p className="ask-question">{str(choice.question)}</p>
          <table className="list decision-options">
            <tbody>
              {options.map((o, i) => {
                const name = str(o.name);
                return (
                  <tr key={i} className={name === recommended ? 'chosen' : undefined}>
                    <td>
                      <strong>{name}</strong>
                      {name === recommended ? <div className="good">{t('ask_recommended')}</div> : null}
                    </td>
                    <td className="prose">{str(o.does)}</td>
                    <td className="prose muted">{str(o.cost_risk)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {choice.confidence !== undefined ? (
            <Text label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
          ) : null}
        </>
      ) : null}
      {points.length === 0 && choice === null ? <Json value={a.body} /> : null}
      {answer !== null ? (
        <div className="decision-fact">
          <span className="decision-label">
            {t('ask_answer_by', { by: str(answer.by) })}
            {typeof answer.at === 'number' ? ` · ${time(answer.at)}` : ''}
          </span>
          <Written text={str(answer.text)} />
        </div>
      ) : (
        <AnswerForm a={a} options={options.map((o) => str(o.name)).filter((o) => o !== '')} />
      )}
    </div>
  );
}
