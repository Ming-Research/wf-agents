import { describe, expect, it } from 'vitest';

import type { Run, Step } from '../works-api';
import { fixtureItems, fixtureProjects, fixtureRound, fixtureTold, fixtureVersion, fixtureWork } from './fixtures';
import { isWayRound } from './words';

const runs = (steps: Step[]) => steps.filter((s): s is Step & Run & { kind: 'run' } => s.kind === 'run');

describe('the fixtures', () => {
  it('hold one work for each item, and a work of each kind the page must show', () => {
    const works = fixtureItems().items.map((i) => i.work);
    expect(fixtureItems().items.map((i) => i.work.item)).toEqual(fixtureItems().items.map((i) => i.number));
    const nows = works.map((w) => w.now);
    expect(nows.some((n) => n.kind === 'round')).toBe(true);
    expect(nows.some((n) => n.kind === 'waits' && n.on.some((w) => w.kind === 'call' && w.what.kind === 'atLimit'))).toBe(true);
    expect(nows.some((n) => n.kind === 'waits' && n.on.some((w) => w.kind === 'limit'))).toBe(true);
    expect(nows.some((n) => n.kind === 'waits' && n.on.some((w) => w.kind === 'precondition'))).toBe(true);
    expect(works.some((w) => w.paused && w.now.kind === 'paused' && w.now.by === 'owner')).toBe(true);
    expect(works.some((w) => w.paused && w.now.kind === 'paused' && w.now.by === 'engine' && w.now.why !== null)).toBe(true);
    expect(works.some((w) => w.cells.some((c) => c.state === 'failed'))).toBe(true);
    expect(nows.some((n) => n.kind === 'merged')).toBe(true);
    expect(nows.some((n) => n.kind === 'cancelled')).toBe(true);
  });

  it('are of as many projects as asked, three unless asked, each item of one of them or of none, as its work says', () => {
    const of = (projects?: number) => {
      const names = fixtureProjects(projects).names;
      const items = fixtureItems(projects).items;
      // Every item and its work's page say the same project, one the installation names, or none.
      for (const item of items) {
        expect(fixtureWork(item.work.number, projects).project).toBe(item.project);
        if (item.project !== null) expect(names).toContain(item.project);
      }
      return { names, used: new Set(items.map((i) => i.project)) };
    };
    expect(of().names).toEqual(['Whitefoot', 'example', 'wf-agents']);
    // Every project has a work, and one item has none.
    expect(of().used).toEqual(new Set(['Whitefoot', 'example', 'wf-agents', null]));
    expect(of(1)).toEqual({ names: ['wf-agents'], used: new Set(['wf-agents', null]) });
    // From eight projects on, a project has no work, as a project the installation has just named.
    const many = of(12);
    expect(many.names).toHaveLength(12);
    expect(many.names).toEqual([...many.names].sort());
    expect(many.names.filter((n) => !many.used.has(n))).toEqual(['project-10', 'project-11', 'project-12', 'project-8', 'project-9']);
  });

  it('hold a long history with a return sent back, tools’ results, an abandoned run, ways round and a write by the outside', () => {
    const page = fixtureWork(41);
    const all = runs(page.steps);
    // One round with what it was told and what it returned.
    const told = all.flatMap((r) => r.rounds.map((d) => fixtureTold(41, r.run, d.number))).filter((t) => t.tries.length > 0);
    expect(told).toHaveLength(1);
    expect(told[0].tries[0].back.kind).toBe('returned');
    expect(all.some((r) => r.rounds.some((d) => d.given.kind === 'sentBack'))).toBe(true);
    expect(all.some((r) => r.calls.some((c) => c.what.kind === 'tool' && c.done !== null))).toBe(true);
    // A question for a person, which only a work recorded before the engine stopped making them holds, is still read.
    expect(all.some((r) => r.calls.some((c) => c.what.kind === 'question'))).toBe(true);
    expect(all.some((r) => r.end?.kind === 'abandoned')).toBe(true);
    expect(all.filter((r) => r.cell === 'implement' && isWayRound(r))).toHaveLength(2);
    expect(page.steps.some((s) => s.kind === 'wrote' && s.by.kind === 'outside')).toBe(true);
  });

  it('hold a work paused by the engine as its round failed: the round failed with the reason, the pause after it', () => {
    const page = fixtureWork(50);
    const implement = runs(page.steps).find((r) => r.cell === 'implement');
    const last = implement?.rounds.at(-1);
    expect(last?.back).toEqual({
      kind: 'failed',
      why: page.summary.now.kind === 'paused' ? page.summary.now.why : null,
      failure: 'other',
      resets: null,
    });
    expect(last?.given.kind).toBe('results');
    expect(page.steps.at(-1)).toMatchObject({ kind: 'paused', by: 'engine', time: last?.ended });
  });

  it('agree with themselves: each version and each round handed in can be read', () => {
    for (const item of fixtureItems().items) {
      {
        const work = item.work;
        const page = fixtureWork(work.number);
        for (const block of page.blocks) {
          for (const v of block.versions) {
            expect(fixtureVersion(work.number, block.name, v.number).content).toHaveLength(v.chars);
          }
        }
        for (const run of runs(page.steps)) {
          for (const round of run.rounds) {
            const full = fixtureRound(work.number, run.run, round.number);
            if (round.back?.kind === 'handedIn') expect(full.handed).toHaveLength(round.back.blocks.length);
          }
        }
      }
    }
  });
});
