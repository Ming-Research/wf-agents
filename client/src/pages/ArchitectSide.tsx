// The panel beside the owner's one-to-one with a project's architect, which the owner closes and opens again, as this
// browser keeps it, in tabs: the architect's notebook, each of its four sections (what the project is now, the plan,
// what is unclear, anything else) as it stands, under the round and cell that last wrote it, with its earlier versions
// folded under it, newest first; its policy, the draft that waits for the owner's approval on top while one does, then
// the policy in force with the project's constitution folded under it; and its queue, how many matters are left, how
// long the oldest waited and how many it handled today, over the matters in the order they came, the one in hand
// marked, and those an ask of the owner holds, then those it handled with nothing to do, each with why folded, which
// the one-to-one no longer holds once done. Each part names the cell that wrote it, and its round opens on the
// architect's chart.

import { Label } from '@primer/react';
import { useEffect, useState } from 'react';

import type { Architect, NotebookSection } from '../api';
import { architectState, queueParts } from '../architect';
import { duration, time } from '../format';
import { Markdown } from '../markdown';
import { Block, By, Fold, type OpenRound, Section } from '../parts';
import { architectHref, href, Link, navigate, type SideTab, sideHref } from '../route';
import { type Key, t, tn } from '../text';
import { joinedWords } from '../voices';
import { PolicyText } from './ArchitectViews';
import { MatterWhat } from './TalkCards';
import { ChartPanel } from './WorkChart';

/// What the panel is open or closed as, kept in this browser for every one-to-one: open, as at first, or closed.
const SIDE_KEY = 'wf-agents.architect-side';

/// Whether the panel is open, as this browser kept it, open where nothing is kept or the browser keeps nothing; and how
/// to open or close it, which the browser keeps where it can. An address that names a tab of the panel opens it.
export function useSideOpen(named: boolean): [boolean, (open: boolean) => void] {
  const [open, setOpen] = useState(() => named || sideKept());
  // An address that comes to name a tab opens the panel.
  const [wasNamed, setWasNamed] = useState(named);
  if (named !== wasNamed) {
    setWasNamed(named);
    if (named) setOpen(true);
  }
  useEffect(() => {
    if (named) keepSide(true);
  }, [named]);
  const set = (on: boolean) => {
    setOpen(on);
    keepSide(on);
  };
  return [open, set];
}

/// Whether this browser kept the panel open: open where it kept nothing, or keeps nothing.
export function sideKept(): boolean {
  try {
    return localStorage.getItem(SIDE_KEY) !== 'closed';
  } catch {
    return true;
  }
}

/// Keeps in this browser whether the panel is open; where the browser keeps nothing, the panel stays as set until the
/// page is opened again.
export function keepSide(open: boolean): void {
  try {
    localStorage.setItem(SIDE_KEY, open ? 'open' : 'closed');
  } catch {
    // Nothing is kept where the browser lets nothing be.
  }
}

/// The sections of the notebook, by their names, in the words the owner reads them in.
const SECTIONS: Record<string, Key> = {
  project_now: 'notebook_project_now',
  plan: 'notebook_plan',
  unclear: 'notebook_unclear',
  other: 'notebook_other',
};

/// One section of the notebook, folded at first as the owner asked: its text as it stands, under the round and cell
/// that wrote it, or that nothing wrote it yet; and, folded under it, each earlier version, newest first, under the
/// round and cell that wrote it and when.
function NotebookPart({ s, onRound }: { s: NotebookSection; onRound: OpenRound }) {
  const now = s.now;
  const earlier = s.versions.filter((v) => now === null || v.work !== now.work || v.round !== now.round);
  const key = SECTIONS[s.section];
  return (
    <Section
      title={key !== undefined ? t(key) : s.section}
      by={now !== null ? <By cell={now.cell} at={now.at} work={now.work} round={now.round} onRound={onRound} /> : undefined}
    >
      {now !== null ? (
        <div className="document">
          <Markdown text={now.text} />
        </div>
      ) : (
        <p className="muted">{t('notebook_unwritten')}</p>
      )}
      {earlier.length > 0 ? (
        <Section title={t('notebook_earlier')} n={earlier.length}>
          {earlier.map((v) => (
            <Block key={`${v.work}-${v.round}`} by={<By cell={v.cell} at={v.at} work={v.work} round={v.round} onRound={onRound} />}>
              <Markdown text={v.text} />
            </Block>
          ))}
        </Section>
      ) : null}
    </Section>
  );
}

/// The architect's notebook, its four sections in their order.
function Notebook({ a, onRound }: { a: Architect; onRound: OpenRound }) {
  return (
    <>
      {a.notebook.map((s) => (
        <NotebookPart key={s.section} s={s} onRound={onRound} />
      ))}
    </>
  );
}

/// The architect's policy: the draft that waits for the owner's approval, while one does, headed as such, as one
/// document under the round that wrote it; the policy in force, as one document, with when it took effect and the
/// round that wrote it, or that none has taken effect yet; and under it the project's constitution, which the
/// architect judges by with the policy, as the project's own file has it in its checkout; each folded at first, as the
/// owner asked. The owner approves the draft by replying to the architect's question in the one-to-one beside it.
function Policy({ a, onRound }: { a: Architect; onRound: OpenRound }) {
  const p = a.policy;
  const draft = a.draft;
  const law = a.constitution;
  return (
    <>
      {draft !== null ? (
        <Section
          title={t('policy_draft')}
          by={<By at={draft.at} work={draft.work} round={draft.round} onRound={onRound} />}
        >
          <PolicyText value={draft.policy} />
        </Section>
      ) : null}
      <Section
        title={t('policy_in_force')}
        by={
          p !== null ? (
            <By cell={p.cell} work={p.work} round={p.round} onRound={onRound}>
              {t('policy_took_effect', { at: time(p.at) })}
            </By>
          ) : undefined
        }
      >
        {p !== null ? <PolicyText value={p.policy} /> : <p className="muted">{t('policy_none')}</p>}
      </Section>
      <Section
        title={t('constitution')}
        by={law !== null ? <span className="by">{t('constitution_at', { path: law.name })}</span> : undefined}
      >
        {law === null ? (
          <p className="muted">{t('constitution_none')}</p>
        ) : law.text.trim() === '' ? (
          <p className="muted">{t('constitution_unread', { path: law.name })}</p>
        ) : (
          <div className="document">
            <Markdown text={law.text} />
          </div>
        )}
      </Section>
    </>
  );
}

/// The start of today in the browser's time zone.
const today = (): number => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

/// The architect's queue in a few words: how many matters are left, how long the oldest has waited, how many it handled
/// today, and how many an ask of the owner holds.
function QueueFacts({ a }: { a: Architect }) {
  const { waiting, held } = queueParts(a.queue);
  const oldest = waiting.reduce<number | null>((m, x) => (m === null || x.at < m ? x.at : m), null);
  const from = today();
  const done = a.handled.filter((h) => h.at !== null && h.at >= from).length;
  const [now] = useState(() => Date.now());
  return (
    <p className="muted">
      {joinedWords([
        tn('talk_queue_left', waiting.length),
        oldest !== null ? t('talk_queue_oldest', { for: duration(now - oldest) }) : null,
        t('talk_queue_today', { n: done }),
        held.length > 0 ? tn('talk_queue_held', held.length) : null,
      ])}
    </p>
  );
}

/// The architect's queue: the matters left in the order they came, the one in hand marked, and those an ask of the
/// owner holds.
function QueueList({ a }: { a: Architect }) {
  const { waiting, held } = queueParts(a.queue);
  const state = architectState(a.works, a.queue);
  const handling = state.kind === 'working' ? (state.handling?.id ?? null) : null;
  return (
    <ol className="talk-queue-list">
      {[...waiting, ...held].map((m) => (
        <li key={m.id} className={m.id === handling ? 'talk-queue-current' : undefined}>
          <MatterWhat m={m} /> <span className="talk-card-facts">{time(m.at)}</span>
          {m.id === handling ? (
            <>
              {' '}
              <Label variant="accent">{t('matter_working')}</Label>
            </>
          ) : null}
          {held.includes(m) ? (
            <>
              {' '}
              <Label variant="attention">{t('matter_held')}</Label>
            </>
          ) : null}
          {m.work !== null && m.source === 'work' ? (
            <>
              {' '}
              <Link to={href({ page: 'messages', open: `work:${m.work}` })}>{t('matter_in_group', { work: m.work })}</Link>
            </>
          ) : null}
        </li>
      ))}
    </ol>
  );
}

/// The matters the architect handled with nothing to do, newest first, which leave the one-to-one once done: each its
/// kind and what it asked, when, and why nothing came of it folded under it.
function HandledNothing({ a }: { a: Architect }) {
  const nothing = a.handled.filter((h) => h.nothing);
  if (nothing.length === 0) return null;
  return (
    <Section title={t('queue_handled_nothing')} n={nothing.length}>
      <ol className="talk-queue-list">
        {nothing.map((h) => (
          <li key={h.matter}>
            <MatterWhat m={{ ...h, fix: null }} /> <span className="talk-card-facts">{time(h.at)}</span>
            {h.why !== null ? (
              <Fold summary={t('queue_handled_why')}>
                <Markdown text={h.why} />
              </Fold>
            ) : null}
          </li>
        ))}
      </ol>
    </Section>
  );
}

/// The panel beside the one-to-one with an architect, in tabs: its notebook, its policy and its queue, the queue's tab
/// counting the matters left; a tab changes the address, and the close closes the panel.
export function ArchitectSide({ a, tab, onClose }: { a: Architect; tab: SideTab; onClose: () => void }) {
  const onRound: OpenRound = (work, round) => navigate(architectHref({ project: a.project, work, round }));
  const tabs: Array<{ tab: SideTab; label: string; n: number | null }> = [
    { tab: 'notebook', label: t('side_tab_notebook'), n: null },
    { tab: 'policy', label: t('arch_tab_policy'), n: null },
    { tab: 'queue', label: t('side_tab_queue'), n: queueParts(a.queue).waiting.length },
  ];
  return (
    <ChartPanel
      label={t('side_tabs')}
      current={tab}
      onTab={(x) => navigate(sideHref(a.project, x), true)}
      onClose={onClose}
      tabs={tabs.map((x) => ({ ...x, href: sideHref(a.project, x.tab) }))}
    >
      {tab === 'notebook' ? <Notebook a={a} onRound={onRound} /> : null}
      {tab === 'policy' ? <Policy a={a} onRound={onRound} /> : null}
      {tab === 'queue' ? (
        <>
          <QueueFacts a={a} />
          <QueueList a={a} />
          <HandledNothing a={a} />
        </>
      ) : null}
    </ChartPanel>
  );
}
