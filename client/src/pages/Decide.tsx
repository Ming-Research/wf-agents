// What the owner decides on the page: design cards and asks of the owner, which share one card wherever the page
// shows them (a work's tabs, an item's page, the round panel): a frame whose head says which card it is, its status as
// a chip and the round it came from; a band that says where it stands with the owner; and boxes that hold what the
// agent wrote, word for word, each under a label of the page's, with option boxes for approaches and options alike.
// A design card holds each decision as a numbered fold: its change and kind as chips and its node; then the decision
// (or what it was and what it becomes, or what it removes), why, what is provisional or to check, the approaches
// compared with the chosen one first, how sure and what would overturn it. While it waits for a ruling, the owner rules
// on each decision (confirm, disagree with a reason, or click another of its approaches to choose it) and sends the
// verdicts on all of them at once, after a summary of what is sent. A replacing card says first what it changed from
// the one it replaced. An ask of the owner holds what it asks about, each point under its kind coloured by the kind's
// group, and for a decision the options with the one recommended; in its conversation the ask is the asking agent's
// bubble, answered in that bubble by its buttons and a small box of the owner's words. Both write what the command
// line writes, and the engine goes on at once. The owner rules and answers in the messages view alone: wherever else
// a card shows, it says where it waits to be ruled on or answered.

import { CheckIcon, ChevronDownIcon, ChevronRightIcon } from '@primer/octicons-react';
import {
  Button,
  ButtonGroup,
  Flash,
  Label,
  Textarea,
  type FlashProps,
  type LabelColorOptions,
} from '@primer/react';
import { Fragment, useState, type MouseEvent, type ReactNode } from 'react';

import { askChoice, askParts, useAnswersHere } from '../answer';
import type { AskKind, DesignCard, JsonValue, OwnerAsk } from '../api';
import { toneVariant } from '../components';
import { duration, time } from '../format';
import { postJson } from '../http';
import { OptionBoxes } from '../item/Shapes';
import { isObj, Labelled, str, words, type Obj } from '../labelled';
import { reread, useNow } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { RoundAt, SendForm, SubHeading } from '../parts';
import { href, Link, navigate } from '../route';
import { t } from '../text';
import { joined, SEP } from '../voices';

export { words };

/// What the owner decided has been written: everything it shows is read again.
export function decided() {
  reread('works', 'work:*', 'items', 'item:*', 'round:*', 'architect', 'messages');
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

/// A card the page shows the owner: open, its head over its body; folded, its head as the one line that opens it.
export function CardFrame({ id, head, folded = false, children }: {
  id: string;
  head: ReactNode;
  folded?: boolean;
  children: ReactNode;
}) {
  if (folded) {
    return (
      <details className="dc dc-folded" id={id}>
        <summary className="dc-head">
          <ChevronRightIcon className="fold-closed" />
          <ChevronDownIcon className="fold-open" />
          {head}
        </summary>
        <div className="dc-body">{children}</div>
      </details>
    );
  }
  return (
    <div className="dc" id={id}>
      <div className="dc-head">{head}</div>
      <div className="dc-body">{children}</div>
    </div>
  );
}

/// How a card stands, as a chip: done with the owner, waiting for the owner, replaced, or disagreed with.
type Tone = 'ok' | 'wait' | 'old' | 'no';

const CHIP: Record<Tone, LabelColorOptions> = { ok: 'success', wait: 'attention', old: toneVariant('neutral'), no: 'danger' };

const BAND: Record<Tone, NonNullable<FlashProps['variant']>> = {
  ok: 'success',
  wait: 'warning',
  old: 'default',
  no: 'danger',
};

function StatusChip({ tone, children }: { tone: Tone; children: ReactNode }) {
  return <Label variant={CHIP[tone]}>{children}</Label>;
}

/// Where a card stands with the owner, in a band under its head.
function Band({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <Flash variant={BAND[tone]} className="dc-band">
      {children}
    </Flash>
  );
}

/// What an agent wrote, word for word, in a box under a label of the page's and its note; `was` strikes it through,
/// `chg` marks what changes, `warn` what is provisional or to check, and `bare` holds boxes of their own.
function Box({ label, note = null, tone = null, bare = false, children }: {
  label: ReactNode;
  note?: ReactNode;
  tone?: 'was' | 'chg' | 'warn' | null;
  bare?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="dc-f">
      <div className="dc-fl">
        <SubHeading>{label}</SubHeading>
        {note !== null ? <span className="dc-note">{note}</span> : null}
      </div>
      {bare ? children : <div className={`dc-val${tone !== null ? ` dc-val-${tone}` : ''}`}>{children}</div>}
    </div>
  );
}

/// A text the agent wrote, in a box, when it wrote one.
function TextBox({ label, note = null, tone = null, value }: {
  label: ReactNode;
  note?: ReactNode;
  tone?: 'was' | 'chg' | 'warn' | null;
  value: JsonValue | undefined;
}) {
  if (value === undefined || value === null || (typeof value === 'string' && value.trim() === '')) return null;
  return (
    <Box label={label} note={note} tone={tone}>
      {typeof value === 'string' ? <Markdown text={value} /> : <Labelled value={value} />}
    </Box>
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
      <ButtonGroup>
        <Button
          size="small"
          leadingVisual={v.verdict === 'confirm' ? CheckIcon : undefined}
          onClick={() => set({ verdict: 'confirm', text: '', option: '' })}
          aria-pressed={v.verdict === 'confirm'}
        >
          {t('rule_confirm')}
        </Button>
        <Button
          size="small"
          leadingVisual={v.verdict === 'disagree' ? CheckIcon : undefined}
          onClick={() => set({ ...v, verdict: 'disagree' })}
          aria-pressed={v.verdict === 'disagree'}
        >
          {t('rule_disagree')}
        </Button>
      </ButtonGroup>
      {v.verdict === 'disagree' ? (
        <>
          {v.option !== '' ? <p className="muted pick-hint">{t('rule_chosen_option', { option: v.option })}</p> : null}
          <Textarea
            block
            resize="vertical"
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

/// How sure the agent is, as five pips with the number filled.
function Pips({ n }: { n: number }) {
  return (
    <span className="dc-pips" aria-label={`${n}/5`}>
      {[1, 2, 3, 4, 5].map((i) => (
        <span key={i} className={i <= n ? 'dc-pip on' : 'dc-pip'} />
      ))}
    </span>
  );
}

/// The approaches compared, the chosen one first.
function chosenFirst(options: Obj[], chosen: string): Obj[] {
  return [...options.filter((o) => str(o.name) === chosen), ...options.filter((o) => str(o.name) !== chosen)];
}

/// One decision of a design card, a numbered fold: its change and kind as chips and its node, its first sentence as
/// its title while folded; open, in boxes, the decision (what it was and what it becomes for a change, what it removes
/// for a removal), why, what is provisional or to check, the approaches compared with the chosen one first (which the
/// owner may click to choose one for this decision), how sure and what would overturn it.
function Decision(props: { d: Obj; n: number; v: Verdict | null; set: ((v: Verdict) => void) | null }) {
  const { d, n, v, set } = props;
  const change = str(d.change);
  const settled = str(d.settled);
  const chosen = str(d.chosen);
  const options = chosenFirst(Array.isArray(d.options) ? d.options.filter(isObj) : [], chosen);
  const sure = Number(str(d.sure));
  const decision = change === 'remove' ? d.before : d.after;
  const pick = set !== null && v !== null ? (name: string) => set(pickApproach(v, name, chosen)) : null;
  const warn = settled === 'provisional' || settled === 'to_check' ? 'warn' : null;
  return (
    <details className="dc-dec" open>
      <summary>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        <span className="dc-dn">{n}</span>
        {change !== '' ? (
          <Label variant="secondary">
            {change === 'add' ? t('dc_new') : change === 'remove' ? t('dc_removed') : t('dc_changed')}
          </Label>
        ) : null}
        {settled !== '' ? (
          <Label variant={settled === 'settled' ? 'success' : 'attention'}>
            {settled === 'settled' ? t('dc_settled') : settled === 'provisional' ? t('dc_provisional') : words(settled)}
          </Label>
        ) : null}
        {str(d.node) !== '' ? <code className="dc-node">{str(d.node)}</code> : null}
        <span className="dc-dt">
          <MarkdownInline text={firstSentence(str(decision))} />
        </span>
      </summary>
      <div className="dc-db">
        {change === 'change' ? (
          <>
            <TextBox label={t('dc_before')} tone="was" value={d.before} />
            <TextBox label={t('dc_after')} note={t('dc_decision_note')} tone="chg" value={d.after} />
          </>
        ) : change === 'remove' ? (
          <TextBox label={t('dc_remove')} tone="was" value={d.before} />
        ) : (
          <TextBox label={t('dc_decision')} note={t('dc_decision_note')} value={d.after} />
        )}
        <TextBox label={t('dc_why')} note={t('dc_why_note')} value={d.why} />
        <TextBox label={t('decision_uncertain')} tone={warn} value={d.uncertain} />
        <TextBox label={t('decision_reopen')} tone={warn} value={d.reopen} />
        <TextBox label={t('decision_assume')} tone={warn} value={d.assume} />
        <TextBox label={t('decision_check')} tone={warn} value={d.check} />
        <TextBox label={t('decision_carried_by')} value={d.carried_by} />
        <TextBox label={t('decision_from_parent')} value={d.from_parent} />
        {options.length > 0 ? (
          <Box label={t('dc_approaches')} note={t('dc_approaches_note', { n: options.length })} bare>
            <OptionBoxes
              options={options}
              fields={[
                ['does', t('option_does')],
                ['verdict', t('dc_why_not'), t('dc_why_chosen')],
              ]}
              recommended={chosen}
              mark={t('dc_chosen')}
              picked={v?.option ?? ''}
              onPick={pick}
            />
          </Box>
        ) : null}
        {sure >= 1 && sure <= 5 ? (
          <Box
            label={t('dc_sure')}
            note={
              <>
                <Pips n={sure} /> {sure}/5{SEP}{t(`sure_${sure}` as 'sure_1')}
              </>
            }
          >
            <Markdown text={str(d.sure_why)} />
          </Box>
        ) : null}
        <TextBox label={t('decision_overturn')} value={d.overturn} />
        {set !== null && v !== null ? <RuleDecision v={v} set={set} /> : null}
      </div>
    </details>
  );
}

/// The verdicts of a ruling, each decision's, or for one made before rulings were per decision, the card's.
function verdictsOf(r: Obj): string[] {
  const each = Array.isArray(r.decisions) ? r.decisions.filter(isObj) : null;
  if (each === null) return [str(r.verdict)];
  return each.map((d) => str(d.verdict));
}

/// How a design card stands: waiting for a ruling, replaced, disagreed with in its latest ruling, or confirmed.
function cardTone(c: DesignCard): Tone {
  if (c.status === 'open') return 'wait';
  if (c.status === 'replaced') return 'old';
  const list = Array.isArray(c.rulings) ? c.rulings.filter(isObj) : [];
  const last = list[list.length - 1];
  return last !== undefined && verdictsOf(last).some((x) => x === 'disagree') ? 'no' : 'ok';
}

/// The rulings given, each on a line of the band: who ruled and when, and the verdict on each decision.
function RulingBand({ c }: { c: DesignCard }) {
  const list = Array.isArray(c.rulings) ? c.rulings.filter(isObj) : [];
  if (list.length === 0) {
    return c.status === 'open' ? <Band tone="wait">{t('dc_band_wait')}</Band> : null;
  }
  const last = list[list.length - 1];
  return (
    <Band tone={last !== undefined && verdictsOf(last).includes('disagree') ? 'no' : 'ok'}>
      {list.map((r, i) => {
        const each = Array.isArray(r.decisions) ? r.decisions.filter(isObj) : null;
        const lines =
          each !== null
            ? each.map((d) => verdictLine(Number(d.decision), str(d.verdict), str(d.option), str(d.text)))
            : [`${str(r.verdict)}${r.option ? ` · ${t('ruling_option', { option: str(r.option) })}` : ''}${str(r.text) !== '' ? `: ${str(r.text)}` : ''}`];
        const who = str(r.by) === 'owner' ? t('dc_you') : str(r.by);
        return (
          <div key={i}>
            {typeof r.at === 'number' ? t('dc_ruled_at', { who, at: time(r.at) }) : t('dc_ruled', { who })} — {lines.join('; ')}
          </div>
        );
      })}
    </Band>
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
    <SendForm
      above={
        <div className="rule-summary">
          <SubHeading>{t('rule_summary_title')}</SubHeading>
          <ul>
            {verdicts.map((v, i) => (
              <li key={i} className={verdictReady(v) ? undefined : 'muted'}>
                {verdictLine(i + 1, v.verdict, v.option, v.text)}
              </li>
            ))}
          </ul>
        </div>
      }
      label={t('rule_send')}
      ready={ready}
      busy={busy}
      why={t('rule_not_ready')}
      onSend={send}
      error={error}
    />
  );
}

/// Where a card waiting for the owner is ruled on or answered: the conversation it is in, the work's group or the
/// one-to-one with the architect that asked; shown wherever else the card shows, which only reads it.
export function AnswerThere({ conversation, what }: { conversation: string; what: 'rule' | 'answer' }) {
  return (
    <p className="answer-there">
      <Link to={href({ page: 'messages', open: conversation })}>
        {t(what === 'rule' ? 'answer_there_rule' : 'answer_there_answer')}
      </Link>
    </p>
  );
}

/// The conversation an ask is in: an architect's one-to-one for what its work or the setting of its policy asked, else
/// the group of the work that asked.
export function askConversation(a: OwnerAsk): string {
  const body = isObj(a.body) ? a.body : {};
  return body.from === 'architect' && typeof body.project === 'string' ? `architect:${body.project}` : `work:${a.work}`;
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

/// A design card as the owner reads and rules on it, in the card every card shares: its head (its number, how it
/// stands, the round that handed it in at its cell, when, the card it replaced, and for a replaced one the card
/// that replaced it); the band of its rulings; what it changed from the card it replaced; each decision; and while it
/// waits, what the owner sends. `round` is the link to the round that handed it in; a folded card is one line.
export function DesignCardView({ c, round, folded = false }: { c: DesignCard; round: ReactNode; folded?: boolean }) {
  const tone = cardTone(c);
  const decisions = Array.isArray(c.body) ? c.body.filter(isObj) : isObj(c.body) ? [c.body] : [];
  const can = useAnswersHere();
  const open = c.status === 'open' && can;
  const [set, setSet] = useState<Verdict[]>([]);
  const given = decisions.map((_, i) => set[i] ?? UNSET);
  const setOne = (i: number) => (v: Verdict) => setSet(() => given.map((x, j) => (j === i ? v : x)));
  const r = c.replaced;
  const when = c.at !== null ? time(c.at) : '';
  const head = (
    <>
      <span className="dc-name">{t('dc_name', { id: c.id })}</span>
      <StatusChip tone={tone}>
        {tone === 'wait' ? t('dc_waits') : tone === 'old' ? t('dc_replaced') : tone === 'no' ? t('dc_disagreed') : t('dc_confirmed')}
      </StatusChip>
      <RoundAt round={round} cell={c.cell} title={c.title} />
      <span className="dc-note">
        {joined([
          when,
          c.replaces !== null ? (
            <>
              {t('dc_replaces')} <CardLink id={c.replaces} item={c.item} />
            </>
          ) : null,
          c.status !== 'replaced' ? null : r !== null ? (
            <>
              {t('dc_replaced_by')} <CardLink id={r.design_card} item={c.item} />{' '}
              {t('dc_replaced_at', { round: `R${r.work}-${r.round}` })}
            </>
          ) : (
            t('card_replaced_none')
          ),
          c.status === 'replaced' && r !== null && r.at !== null ? time(r.at) : null,
        ]).map((x, k) => (
          <Fragment key={k}>{x}</Fragment>
        ))}
      </span>
    </>
  );
  return (
    <CardFrame id={`design-card-${c.id}`} head={head} folded={folded}>
      <RulingBand c={c} />
      {c.replaces !== null && c.changed !== null && c.changed.trim() !== '' ? (
        <TextBox
          label={t('dc_changed_from', { id: c.replaces })}
          note={t('dc_changed_note')}
          tone="chg"
          value={c.changed}
        />
      ) : null}
      {decisions.length === 0 ? <Labelled value={c.body} /> : null}
      {decisions.map((d, i) => (
        <Decision key={i} d={d} n={i + 1} v={open ? given[i] : null} set={open ? setOne(i) : null} />
      ))}
      {open && decisions.length > 0 ? <RuleForm c={c} verdicts={given} /> : null}
      {c.status === 'open' && !can ? <AnswerThere conversation={`work:${c.work}`} what="rule" /> : null}
    </CardFrame>
  );
}

/// Which of a list of design cards is open at first: the one waiting for a ruling, else the newest that was not
/// replaced; every other is folded to its one line.
export function currentCard(cards: DesignCard[]): number | null {
  const open = cards.find((c) => c.status === 'open');
  if (open !== undefined) return open.id;
  const live = cards.filter((c) => c.status !== 'replaced');
  return live.length > 0 ? Math.max(...live.map((c) => c.id)) : null;
}

/// The kind of a point an ask is about, as a chip coloured by the kind's group, so that the sort of question shows at
/// a glance.
export function KindChip({ k }: { k: AskKind }) {
  return (
    <Label
      className={`kind kind-${k.group ?? 'none'}`}
      title={k.group !== null ? t('kind_group', { group: words(k.group) }) : undefined}
    >
      {words(k.kind)}
    </Label>
  );
}

/// The part of an ask's card about the owner's answer: none in a conversation, where what answers the ask and the
/// answer once given are in its bubble; elsewhere the answer once given (`given`), or where the ask waits to be
/// answered.
export function AnswerPart({ a, given }: { a: OwnerAsk; given: ReactNode }) {
  if (useAnswersHere()) return null;
  return a.answer === null ? <AnswerThere conversation={askConversation(a)} what="answer" /> : <>{given}</>;
}

/// A text an agent wrote, as it reads.
export function Written({ text }: { text: string }) {
  return <Markdown text={text} />;
}

/// How long an ask that waits for the owner's answer has waited since the owner was told of it, ticking; nothing once it
/// is answered.
export function AskWaited({ a }: { a: OwnerAsk }) {
  const waits = a.answer === null;
  const now = useNow(waits);
  if (!waits) return null;
  return (
    <span className="muted" title={t('ask_waited_since', { at: time(a.asked_at) })}>
      {t('ask_waited', { time: duration(now - a.asked_at) })}
    </span>
  );
}

/// An ask of the owner as the owner reads and answers it, in the card every card shares: its head (its number, whether
/// it waits and for how long, the round that asked); a band saying who answered and when, or that it waits; for each point, its kind as
/// a chip, what it says and what was tried, in boxes; the choice, when there is one, its question and its options in
/// option boxes with the one recommended; then, but in a conversation, the answer, or where it waits to be answered.
/// `round` is the link to the round that asked.
export function AskView({ a, round = null, folded = false }: { a: OwnerAsk; round?: ReactNode; folded?: boolean }) {
  const body = isObj(a.body) ? a.body : {};
  const held = askParts(body);
  const points = held.flatMap((x) => (Array.isArray(x.points) ? x.points.filter(isObj) : []));
  const choice = askChoice(body);
  const options = choice !== null && Array.isArray(choice.options) ? choice.options.filter(isObj) : [];
  const answer = a.answer !== null && isObj(a.answer) ? a.answer : null;
  const kind = (k: string): AskKind => a.kinds.find((x) => x.kind === k) ?? { kind: k, group: null };
  const waits = answer === null;
  const head = (
    <>
      <span className="dc-name">{t('ask_head', { id: a.id })}</span>
      <StatusChip tone={waits ? 'wait' : 'ok'}>{waits ? t('dc_waits') : t('ask_answered')}</StatusChip>
      <AskWaited a={a} />
      {round}
      {body.to === 'architect' ? <span className="dc-note">{t('ask_for_architect')}</span> : null}
    </>
  );
  const by = answer !== null ? (str(answer.by) === 'owner' ? t('dc_you') : str(answer.by)) : '';
  return (
    <CardFrame id={`ask-${a.id}`} head={head} folded={folded}>
      {answer !== null ? (
        <Band tone="ok">
          {typeof answer.at === 'number' ? t('dc_answered_at', { who: by, at: time(answer.at) }) : t('dc_answered', { who: by })}
        </Band>
      ) : (
        <Band tone="wait">{t('dc_band_ask_wait')}</Band>
      )}
      {points.map((p, i) => (
        <div key={i} className="dc-point">
          <KindChip k={kind(str(p.kind))} />
          <TextBox label={t('dc_what')} value={p.what} />
          <TextBox label={t('ask_tried_label')} value={p.tried} />
        </div>
      ))}
      {choice !== null ? (
        <>
          <TextBox label={t('dc_question')} value={choice.question} />
          {options.length > 0 ? (
            <Box label={t('dc_options')} note={t('dc_options_note', { n: options.length })} bare>
              <OptionBoxes
                options={options}
                fields={[
                  ['does', t('option_does')],
                  ['cost_risk', t('option_cost')],
                ]}
                recommended={str(choice.recommended)}
                mark={t('ask_recommended')}
              />
            </Box>
          ) : null}
          {choice.confidence !== undefined ? (
            <TextBox label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
          ) : null}
        </>
      ) : null}
      {points.length === 0 && choice === null ? <Labelled value={a.body} /> : null}
      <AnswerPart a={a} given={answer !== null ? <TextBox label={t('dc_answer')} value={answer.text} /> : null} />
    </CardFrame>
  );
}
