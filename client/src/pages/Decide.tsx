// What the owner decides on the page: decision cards and asks of the owner, which share one card wherever the page
// shows them (an item's design, a work's group, the round panel): a frame whose head says which card it is, where it
// stands with the owner as its status's chip, the round it came from and its facts (who ruled or answered and when,
// or for a decision card that waits that it blocks nothing until the work merges); and boxes that hold what the agent
// wrote, word for word, each under a label of the page's, with option boxes for the options of both. A decision card
// is one question put to the owner: its question, its options as boxes, the one recommended marked, picked by a click
// as an ask's are; those found not workable, folded; how sure, as pips, with why and what would overturn it; the
// records it changes, folded, each with its words; and, while it waits, a box of the owner's words with Confirm, which
// becomes Choose on another option picked, Disagree, and Ask, which asks about it, to the architect unless the owner
// ticks the engineer, and rules nothing. It is ruled and asked about in its stage's bubble of the work's group, the
// messages view being the one place the owner acts, and links to the questions about it and their answers there; in its item's design it stands at its anchor read only, saying
// where it is ruled. Its whole body is filled with the tint of where it stands, inside a thick frame. A replacing card says first what it changed from the one it
// replaced, and a card ruled more than once keeps its earlier rulings in a fold; a design card of a flow version before
// decision cards is drawn as it was, each decision a numbered fold, read only. An ask of the owner holds what it asks
// about, and for a decision the options with the one recommended; in its conversation the ask is the asking agent's
// bubble, answered in that bubble by its buttons and a small box of the owner's words. Both write what the command line
// writes, and the engine goes on at once.

import { ChevronDownIcon, ChevronRightIcon, ReplyIcon } from '@primer/octicons-react';
import { Button, Checkbox, FormControl, Label, Textarea, type LabelColorOptions } from '@primer/react';
import { Fragment, useContext, useState, type MouseEvent, type ReactNode } from 'react';

import { askChoice, askParts, CardTalk, pickAfter, useAnswersHere, useAskPick } from '../answer';
import type { DesignCard, JsonValue, OwnerAsk } from '../api';
import { toneVariant } from '../components';
import { DocList } from '../document';
import { duration, time } from '../format';
import { postJson } from '../http';
import { OptionBoxes } from '../item/Shapes';
import { isObj, label, Labelled, str, words, type Obj } from '../labelled';
import { reread, useNow } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { Fold, LinkButton, RoundAt, SubHeading } from '../parts';
import { href, Link, navigate } from '../route';
import { t, tn } from '../text';
import { joined, SEP, Sheet } from '../voices';

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
/// `look` is a decision card's own look, after where it stands with the owner.
export function CardFrame({ id, head, folded = false, look = null, children }: {
  id: string;
  head: ReactNode;
  folded?: boolean;
  look?: string | null;
  children: ReactNode;
}) {
  const more = look !== null ? ` ${look}` : '';
  if (folded) {
    return (
      <details className={`dc dc-folded${more}`} id={id}>
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
    <div className={`dc${more}`} id={id}>
      <div className="dc-head">{head}</div>
      <div className="dc-body">{children}</div>
    </div>
  );
}

/// How a card, or one decision of it, stands, as a chip: done with the owner, waiting for the owner, replaced or
/// withdrawn, disagreed with, or another of its options chosen.
type Tone = 'ok' | 'wait' | 'old' | 'no' | 'chose';

const CHIP: Record<Tone, LabelColorOptions> = {
  ok: 'success',
  wait: 'attention',
  old: toneVariant('neutral'),
  no: 'danger',
  chose: 'accent',
};

function StatusChip({ tone, children }: { tone: Tone; children: ReactNode }) {
  return <Label variant={CHIP[tone]}>{children}</Label>;
}

/// Who ruled on a card or answered an ask, as a fact names them: the owner as "you".
function whoDid(by: string): string {
  if (by === 'owner') return t('dc_you');
  if (by === 'engine') return t('dc_engine');
  return words(by);
}

/// The owner's words on what a card asked of them, as what is sent to an agent: the option or approach chosen, if any,
/// over the words, if any; nothing when there is neither.
function OwnerWords({ title, choice, text }: { title: string; choice: string; text: string }) {
  if (choice === '' && text.trim() === '') return null;
  return (
    <Box label={title} bare>
      <Sheet>
        {choice !== '' ? (
          <p>
            <strong>
              <MarkdownInline text={choice} />
            </strong>
          </p>
        ) : null}
        {text.trim() !== '' ? <Markdown text={text} /> : null}
      </Sheet>
    </Box>
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

/// Unicode's rules of where a sentence ends (UAX #29), the same in any language.
const SENTENCES = new Intl.Segmenter(undefined, { granularity: 'sentence' });

/// The first sentence of a text, as a title on one line: where Unicode's rules end it, at most 120 characters, cut
/// between characters and never inside one.
export function firstSentence(text: string): string {
  const line = text.trim().split('\n')[0] ?? '';
  // An empty line has no sentence.
  const [first] = SENTENCES.segment(line);
  const one = first?.segment.trim() ?? line;
  const chars = Array.from(one);
  return chars.length > 120 ? `${chars.slice(0, 119).join('')}…` : one;
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

/// The owner's verdict on one decision as a ruling gave it: confirm or disagree, the approach chosen instead and why.
type Ruled = { verdict: string; option: string; text: string };

/// The verdict on each decision of a ruling, by the decision's number; a ruling made before rulings were per decision
/// is the card's, under 0, and holds for every decision.
function verdictsByDecision(r: Obj): Map<number, Ruled> {
  const one = (x: Obj): Ruled => ({ verdict: str(x.verdict), option: str(x.option), text: str(x.text) });
  if (!Array.isArray(r.decisions)) return new Map([[0, one(r)]]);
  return new Map(r.decisions.filter(isObj).map((d) => [Number(d.decision), one(d)]));
}

/// One decision of a design card of before decision cards, read only, a numbered fold: once ruled its verdict as a
/// chip, its change and kind as chips and its node, its first sentence as its title while folded; open, the owner's
/// words on it once ruled, then in boxes the decision (what it was and what it becomes for a change, what it removes
/// for a removal), why, what is provisional or to check, the approaches compared with the chosen one first, the one the
/// owner chose marked, how sure and what would overturn it. `own` says whether the owner's words show here: the ruling
/// was this decision's own, not the card's.
function Decision(props: { d: Obj; n: number; ruled: Ruled | null; own: boolean }) {
  const { d, n, ruled } = props;
  const change = str(d.change);
  const settled = str(d.settled);
  const chosen = str(d.chosen);
  const options = chosenFirst(Array.isArray(d.options) ? d.options.filter(isObj) : [], chosen);
  const sure = Number(str(d.sure));
  const decision = change === 'remove' ? d.before : d.after;
  const warn = settled === 'provisional' || settled === 'to_check' ? 'warn' : null;
  const verdict = ruled === null ? null : ruled.verdict === 'disagree' ? 'no' : ruled.verdict === 'confirm' ? 'ok' : null;
  return (
    <details className="dc-dec" open>
      <summary>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        <span className="dc-dn">{n}</span>
        {verdict !== null ? (
          <StatusChip tone={verdict}>{verdict === 'no' ? t('dc_disagreed') : t('dc_confirmed')}</StatusChip>
        ) : null}
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
        {ruled !== null && props.own ? (
          <OwnerWords title={t('dc_your_ruling')} choice={ruled.option} text={ruled.text} />
        ) : null}
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
              picked={ruled?.option ?? ''}
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
      </div>
    </details>
  );
}

/// The rulings given on a card, the earliest first.
function rulingsOf(c: DesignCard): Obj[] {
  return Array.isArray(c.rulings) ? c.rulings.filter(isObj) : [];
}

/// The owner's rulings on a card, the earliest first.
function ownerRulings(c: DesignCard): Obj[] {
  return rulingsOf(c).filter((r) => str(r.by) === 'owner');
}

/// How a card stands with the owner, as its chip: waiting for a ruling, replaced, withdrawn, confirmed, another option
/// chosen, or disagreed with, by its latest ruling.
function cardTone(c: DesignCard): [Tone, string] {
  if (c.status === 'open') return ['wait', t('dc_waits')];
  if (c.status === 'replaced') return ['old', t('dc_replaced')];
  if (c.status === 'withdrawn') return ['old', t('dc_withdrawn')];
  const last = rulingsOf(c).at(-1);
  if (last === undefined) return ['ok', t('dc_confirmed')];
  const verdicts = [...verdictsByDecision(last).values()];
  if (verdicts.some((v) => v.verdict === 'disagree')) return ['no', t('dc_disagreed')];
  if (verdicts.some((v) => v.verdict === 'choose')) return ['chose', t('dc_chose')];
  return ['ok', t('dc_confirmed')];
}

/// Who ruled and when, as a fact of the card's head.
function ruledFact(r: Obj): string {
  const who = whoDid(str(r.by));
  return typeof r.at === 'number' ? t('dc_ruled_at', { who, at: time(r.at) }) : t('dc_ruled', { who });
}

/// A ruling's verdict, on one decision of a design card of before or on the whole card, in words.
function rulingLine(n: number, v: Ruled): string {
  const head =
    v.verdict === 'choose'
      ? t('ruling_chose', { option: v.option })
      : v.option !== ''
        ? `${words(v.verdict)}, ${t('ruling_option', { option: v.option })}`
        : words(v.verdict);
  const said = v.text.trim() !== '' ? `${head}: ${v.text.trim()}` : head;
  return n === 0 ? said : t('ruling_decision', { n, said });
}

/// The rulings before the latest, in a fold under the card: each who ruled and when, over its verdict.
function EarlierRulings({ list }: { list: Obj[] }) {
  if (list.length === 0) return null;
  return (
    <Fold summary={tn('dc_earlier_rulings', list.length)}>
      {list.map((r, i) => (
        <div key={i} className="rule-summary">
          <span className="dc-note">{ruledFact(r)}</span>
          <ul>
            {[...verdictsByDecision(r)].map(([n, v]) => (
              <li key={n}>{rulingLine(n, v)}</li>
            ))}
          </ul>
        </div>
      ))}
    </Fold>
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

/// A link to a decision card: brought into view where the page shows it, else on its item's page.
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

/// The head of a card: its number, how it stands as its status's chip, while it waits whether it changed since a card
/// of its key the owner ruled on, the round that handed it in at its cell, and its facts: when, the card it replaced,
/// who ruled last and when, while it waits that it blocks nothing until the work merges, for a replaced one the card
/// that replaced it, and for a withdrawn one that it was.
function CardHead({ c, round }: { c: DesignCard; round: ReactNode }) {
  const [tone, says] = cardTone(c);
  const r = c.replaced;
  const last = rulingsOf(c).at(-1) ?? null;
  return (
    <>
      <span className="dc-name">{t('dc_name', { id: c.id })}</span>
      <StatusChip tone={tone}>{says}</StatusChip>
      {c.status === 'open' && c.ruled_before !== null ? (
        <span className="dc-again">{t('dc_changed_since', { id: c.ruled_before })}</span>
      ) : null}
      <RoundAt round={round} cell={c.cell} title={c.title} />
      <span className="dc-note">
        {joined([
          c.at !== null ? time(c.at) : '',
          c.replaces !== null ? (
            <>
              {t('dc_replaces')} <CardLink id={c.replaces} item={c.item} />
            </>
          ) : null,
          last !== null ? ruledFact(last) : null,
          c.status === 'open' ? t('dc_blocks_nothing') : null,
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
}

/// What changed from the card a card replaced, in its agent's words, when it says.
function ChangedFrom({ c }: { c: DesignCard }) {
  if (c.replaces === null || c.changed === null || c.changed.trim() === '') return null;
  return <TextBox label={t('dc_changed_from', { id: c.replaces })} note={t('dc_changed_note')} tone="chg" value={c.changed} />;
}

/// What the owner sends on a decision card: the verdict, and with it the option chosen and the owner's words.
export type CardRuling = { verdict: 'confirm' | 'choose' | 'disagree'; option?: string; text?: string };

/// What a ruling form sends, from the option picked on the card's boxes and the owner's words: the option recommended,
/// or none, confirms; another chooses it; `disagree` disagrees, which needs words and no other option picked. None
/// while it cannot be sent.
export function cardRuling(verdict: 'agree' | 'disagree', picked: string, recommended: string, text: string): CardRuling | null {
  const words = text.trim();
  const said = words !== '' ? { text: words } : {};
  if (verdict === 'disagree') {
    return words !== '' && (picked === '' || picked === recommended) ? { verdict: 'disagree', text: words } : null;
  }
  if (picked !== '' && picked !== recommended) return { verdict: 'choose', option: picked, ...said };
  return { verdict: 'confirm', ...said };
}

/// The ruling of a decision card that waits, under its body: a box of the owner's words full width, then Confirm, or,
/// with another option picked on its boxes, Choose it; and Disagree, which sends the words alone.
function CardRule({ c, picked, recommended }: { c: DesignCard; picked: string; recommended: string }) {
  const [text, setText] = useState('');
  const [to, setTo] = useState<string[]>(['architect']);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const agree = cardRuling('agree', picked, recommended, text);
  const disagree = cardRuling('disagree', picked, recommended, text);
  const failed = (e: unknown) => {
    setBusy(false);
    setError(e instanceof Error ? e.message : String(e));
  };
  const send = (r: CardRuling | null) => {
    if (r === null) return;
    setBusy(true);
    setError(null);
    postJson(`/api/v1/design-cards/${c.id}/rule`, r).then(decided, failed);
  };
  const asked = askedTo(to);
  const ask = () => {
    const question = text.trim();
    if (question === '' || asked === null) return;
    setBusy(true);
    setError(null);
    postJson(`/api/v1/works/${c.work}/say`, { text: question, to: asked, design_card: c.id }).then(() => {
      setBusy(false);
      setText('');
      reread('messages', 'architect', 'work:*');
    }, failed);
  };
  const tick = (x: string, on: boolean) => setTo((now) => (on ? [...now, x] : now.filter((y) => y !== x)));
  return (
    <div className="card-rule" role="group" aria-label={t('card_rule_label', { id: c.id })}>
      <Textarea
        block
        resize="vertical"
        rows={1}
        value={text}
        disabled={busy}
        placeholder={t(agree?.verdict === 'choose' ? 'card_words_choose' : 'card_words')}
        onChange={(e) => setText(e.target.value)}
      />
      <div className="card-rule-actions">
        <Button size="small" variant="primary" disabled={busy || agree === null} onClick={() => send(agree)}>
          {agree?.verdict === 'choose' ? t('card_choose') : t('card_confirm')}
        </Button>
        <Button
          size="small"
          disabled={busy || disagree === null}
          title={disagree === null ? t('card_disagree_needs') : undefined}
          onClick={() => send(disagree)}
        >
          {t('card_disagree')}
        </Button>
        <span className="card-ask" role="group" aria-label={t('card_ask_label', { id: c.id })}>
          {ASK_TO.map((x) => (
            <FormControl key={x} layout="horizontal" disabled={busy}>
              <Checkbox checked={to.includes(x)} onChange={(e) => tick(x, e.target.checked)} />
              <FormControl.Label>{`@${words(x)}`}</FormControl.Label>
            </FormControl>
          ))}
          <Button
            size="small"
            leadingVisual={ReplyIcon}
            disabled={busy || text.trim() === '' || asked === null}
            title={t('card_ask_hint')}
            onClick={ask}
          >
            {t('card_ask')}
          </Button>
        </span>
      </div>
      {error !== null ? <p className="dc-error">{error}</p> : null}
    </div>
  );
}

/// Who the owner's question about a decision card may go to, in the order the card lists them: its project's architect,
/// whom it goes to unless the owner ticks otherwise, and the work's engineer.
const ASK_TO = ['architect', 'engineer'];

/// Who a question about a decision card goes to, by who is ticked: one of them, both, or none while none is.
export function askedTo(ticked: string[]): string | null {
  if (ticked.length === 0) return null;
  return ticked.length > 1 ? 'both' : ticked[0];
}

/// The owner's questions about a decision card and their answers, in the work's group where the card is: one line of
/// links, each bringing that message into view; nothing elsewhere, or while none was asked.
function CardAskedLine({ id }: { id: number }) {
  const talk = useContext(CardTalk);
  const asked = talk?.asked.get(id) ?? [];
  if (talk === null || asked.length === 0) return null;
  return (
    <p className="dc-asked">
      <ReplyIcon /> {t('card_asked')}{' '}
      {asked.map((a, i) => (
        <Fragment key={a.entry}>
          {i > 0 ? SEP : null}
          <LinkButton onClick={() => talk.go(a.entry)}>
            {t(a.answer ? 'card_asked_answer' : 'card_asked_question', { at: time(a.at) })}
          </LinkButton>
        </Fragment>
      ))}
    </p>
  );
}

/// The owner's ruling on a decision card once given: the option that stands (the one recommended, confirmed, or the one
/// chosen) semibold over the owner's words, or the words of a disagreement, in the sheet of what is sent to an agent.
function CardRuled({ r, recommended }: { r: Obj; recommended: string }) {
  const verdict = str(r.verdict);
  const choice = verdict === 'choose' ? str(r.option) : verdict === 'confirm' ? recommended : '';
  return <OwnerWords title={t('dc_your_ruling')} choice={choice} text={str(r.text)} />;
}

/// One record a decision card revises, as the branch had it when the card was posted: its reference, what the branch
/// did to it, and its words.
function RevisedRecord({ r }: { r: Obj }) {
  const change = str(r.change);
  return (
    <div className="dc-record">
      <div className="dc-record-head">
        <code>{str(r.reference)}</code>
        <Label variant="secondary">{change === 'added' ? t('dc_new') : change === 'removed' ? t('dc_removed') : t('dc_changed')}</Label>
      </div>
      <div className={`dc-val${change === 'removed' ? ' dc-val-was' : ''}`}>
        <Markdown text={str(r.text)} />
      </div>
    </div>
  );
}

/// A decision card as the owner reads and rules on it, in the card every card shares, as the mock the owner approved
/// draws it: its head; what changed from the card of its key it replaced; its question, semibold; its options as boxes
/// one under another, each its name, what it does, its cost and its risks, the one recommended marked, picked by a
/// click while it waits; the options considered and found not workable, folded; how sure, as five pips with why and
/// what would overturn it; where it departs from an earlier ruling; the records it changes, folded, each with its
/// words; the flow drawn from the branch, for the card that carries a work's flow; and then, while it waits, in a
/// conversation a box for the owner's words with Confirm, Disagree and Ask, and elsewhere where it is ruled; in a
/// conversation, links to the owner's questions about it and their answers; once ruled, the owner's ruling,
/// the earlier ones folded; once withdrawn, why. Its whole body is filled with the tint of where it stands, inside a
/// thick frame: attention while it waits, success once ruled, neutral once replaced or withdrawn. `round` is the link to
/// the round that handed it in; a folded card is one line.
function DecisionCard({ c, body, round, folded, rules }: {
  c: DesignCard;
  body: Obj;
  round: ReactNode;
  folded: boolean;
  rules: boolean;
}) {
  const [picked, setPicked] = useState('');
  const recommended = str(body.recommended);
  const options = Array.isArray(body.options) ? body.options.filter(isObj) : [];
  const dropped = Array.isArray(body.not_workable) ? body.not_workable.filter(isObj) : [];
  const confidence = isObj(body.confidence) ? body.confidence : {};
  const level = Number(str(confidence.level));
  const records = Array.isArray(c.revision) ? c.revision.filter(isObj) : [];
  const rulings = ownerRulings(c);
  const last = rulings.at(-1) ?? null;
  const open = c.status === 'open' && rules;
  const chosen = last !== null && str(last.verdict) === 'choose' ? str(last.option) : '';
  const look = c.status === 'open' ? 'waits' : c.status === 'ruled' ? 'ruled' : 'old';
  return (
    <CardFrame
      id={`design-card-${c.id}`}
      head={<CardHead c={c} round={round} />}
      folded={folded}
      look={`decision-card ${look}`}
    >
      <ChangedFrom c={c} />
      <p className="dc-question">
        <MarkdownInline text={str(body.question)} />
      </p>
      <OptionBoxes
        options={options}
        fields={[
          ['does', t('option_does')],
          ['cost', t('option_cost_only')],
          ['risks', t('option_risks')],
        ]}
        recommended={recommended}
        mark={t('ask_recommended')}
        picked={open ? picked : chosen}
        onPick={open ? (name) => setPicked((now) => pickAfter(now, name)) : null}
      />
      {dropped.length > 0 ? (
        <Fold summary={t('dc_not_workable', { n: dropped.length })}>
          <DocList items={dropped.map((o) => ({ lead: str(o.name), parts: [[t('dc_why_not_workable'), o.why]] }))} />
        </Fold>
      ) : null}
      {level >= 1 && level <= 5 ? (
        <Box
          label={t('dc_sure')}
          note={
            <>
              <Pips n={level} /> {level}/5{SEP}{t(`sure_${level}` as 'sure_1')}
            </>
          }
        >
          <Markdown text={str(confidence.why)} />
          {str(confidence.overturn) !== '' ? (
            <p className="dc-overturn">
              <strong>{t('dc_overturned_if')}</strong> <MarkdownInline text={str(confidence.overturn)} />
            </p>
          ) : null}
        </Box>
      ) : null}
      <TextBox label={t('dc_departs')} tone="warn" value={body.departs} />
      {records.length > 0 ? (
        <Fold summary={t('dc_records', { n: records.length })}>
          {records.map((r, i) => (
            <RevisedRecord key={i} r={r} />
          ))}
        </Fold>
      ) : null}
      {c.flow !== null ? (
        <p className="dc-flow">
          <Link to={href({ page: 'flow', version: null, graph: null, cell: null, project: null, ref: null, from: c.flow.in_force, snapshot: c.flow.snapshot })}>
            {t('dc_flow')}
          </Link>
        </p>
      ) : null}
      {c.withdrawn !== null ? <TextBox label={t('dc_withdrawn_why')} value={c.withdrawn} /> : null}
      {last !== null ? <CardRuled r={last} recommended={recommended} /> : null}
      <EarlierRulings list={rulings.slice(0, -1)} />
      {open ? <CardRule c={c} picked={picked} recommended={recommended} /> : null}
      <CardAskedLine id={c.id} />
      {c.status === 'open' && !rules ? <AnswerThere conversation={`work:${c.work}`} what="rule" /> : null}
    </CardFrame>
  );
}

/// A design card of a flow version before decision cards, as it was, read only: its head; what it changed from the card
/// it replaced; each decision, with the owner's verdict on it once ruled; the earlier rulings, folded.
function DesignCardBefore({ c, round, folded }: { c: DesignCard; round: ReactNode; folded: boolean }) {
  const decisions = Array.isArray(c.body) ? c.body.filter(isObj) : isObj(c.body) ? [c.body] : [];
  const rulings = rulingsOf(c);
  const last = rulings.at(-1) ?? null;
  const verdicts = last !== null ? verdictsByDecision(last) : new Map<number, Ruled>();
  const whole = verdicts.get(0) ?? null;
  return (
    <CardFrame id={`design-card-${c.id}`} head={<CardHead c={c} round={round} />} folded={folded}>
      <ChangedFrom c={c} />
      {whole !== null ? <OwnerWords title={t('dc_your_ruling')} choice={whole.option} text={whole.text} /> : null}
      {decisions.length === 0 ? <Labelled value={c.body} /> : null}
      {decisions.map((d, i) => (
        <Decision key={i} d={d} n={i + 1} ruled={whole ?? verdicts.get(i + 1) ?? null} own={whole === null} />
      ))}
      <EarlierRulings list={rulings.slice(0, -1)} />
    </CardFrame>
  );
}

/// A decision card wherever the page shows it: one question in the decision card's shape, ruled in a conversation,
/// the one place the owner acts, and elsewhere read only, saying where it is ruled; a design card of before, read
/// only. `round` is the link to the round that handed it in; a folded card is one line.
export function DesignCardView({ c, round, folded = false }: { c: DesignCard; round: ReactNode; folded?: boolean }) {
  const here = useAnswersHere();
  if (!isObj(c.body) || typeof c.body.question !== 'string') {
    return <DesignCardBefore c={c} round={round} folded={folded} />;
  }
  return <DecisionCard c={c} body={c.body} round={round} folded={folded} rules={here} />;
}

/// Which of a list of decision cards is open at first: the one waiting for a ruling, else the newest current one.
export function currentCard(cards: DesignCard[]): number | null {
  const open = cards.find((c) => c.status === 'open');
  if (open !== undefined) return open.id;
  const live = cards.filter((c) => c.status === 'ruled');
  return live.length > 0 ? Math.max(...live.map((c) => c.id)) : null;
}

/// The card of a key a design's anchor stands for, among the cards of the work that wrote the design: its current
/// card, open or ruled, else its newest.
export function cardOfKey(cards: DesignCard[], work: number, key: string): DesignCard | null {
  const mine = cards.filter((c) => c.work === work && c.key === key);
  const current = mine.filter((c) => c.status === 'open' || c.status === 'ruled');
  const from = current.length > 0 ? current : mine;
  return from.length > 0 ? from.reduce((a, b) => (b.id > a.id ? b : a)) : null;
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

/// Who answered an ask and when, as a fact of the card's head.
function answeredFact(answer: Obj): string {
  const who = whoDid(str(answer.by));
  return typeof answer.at === 'number' ? t('dc_answered_at', { who, at: time(answer.at) }) : t('dc_answered', { who });
}

/// An ask's answer once given: the owner's, as the owner's words, the option chosen over them; or, closed by the
/// engine, why no answer was needed, in the page's own words.
function Answer({ answer }: { answer: Obj }) {
  if (str(answer.by) === 'engine') return <p>{str(answer.text)}</p>;
  const choice = str(answer.choice);
  return <OwnerWords title={t('dc_answer')} choice={choice !== '' ? label(choice) : ''} text={str(answer.text)} />;
}

/// An ask of the owner as the owner reads and answers it, in the card every card shares: its head (its number, whether
/// it waits and for how long as its status's chip, the round that asked, and its facts: who answered and when, and
/// whether it is for the architect); for each point, what it asks and what was checked, in boxes, with no kind, which
/// an ask no longer carries and one recorded with a kind before does not show; the choice, when there is one, its
/// question and its options in option boxes with the one recommended and the one the owner chose, which in the bubble
/// that answers it while it waits the owner picks on the boxes themselves; then, but in a conversation, the answer, or
/// where it waits to be answered. `round` is the link to the round that asked.
export function AskView({ a, round = null, folded = false }: { a: OwnerAsk; round?: ReactNode; folded?: boolean }) {
  const picking = useAskPick(a.id);
  const body = isObj(a.body) ? a.body : {};
  const held = askParts(body);
  const points = held.flatMap((x) => (Array.isArray(x.points) ? x.points.filter(isObj) : []));
  const choice = askChoice(body);
  const options = choice !== null && Array.isArray(choice.options) ? choice.options.filter(isObj) : [];
  const answer = a.answer !== null && isObj(a.answer) ? a.answer : null;
  const waits = answer === null;
  const facts = joined([
    answer !== null ? answeredFact(answer) : null,
    body.to === 'architect' ? t('ask_for_architect') : null,
  ]);
  const head = (
    <>
      <span className="dc-name">{t('ask_head', { id: a.id })}</span>
      <StatusChip tone={waits ? 'wait' : 'ok'}>{waits ? t('dc_waits') : t('ask_answered')}</StatusChip>
      <AskWaited a={a} />
      {round}
      {facts.length > 0 ? (
        <span className="dc-note">
          {facts.map((x, k) => (
            <Fragment key={k}>{x}</Fragment>
          ))}
        </span>
      ) : null}
    </>
  );
  return (
    <CardFrame id={`ask-${a.id}`} head={head} folded={folded}>
      {points.map((p, i) => (
        <div key={i} className="dc-point">
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
                picked={answer !== null ? str(answer.choice) : (picking?.picked ?? '')}
                onPick={answer === null ? (picking?.pick ?? null) : null}
              />
            </Box>
          ) : null}
          {choice.confidence !== undefined ? (
            <TextBox label={t('ask_confidence', { n: str(choice.confidence) })} value={choice.confidence_why} />
          ) : null}
        </>
      ) : null}
      {points.length === 0 && choice === null ? <Labelled value={a.body} /> : null}
      <AnswerPart a={a} given={answer !== null ? <Answer answer={answer} /> : null} />
    </CardFrame>
  );
}
