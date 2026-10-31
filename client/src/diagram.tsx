// A diagram written in a text as a fence of its kind: ```mermaid for any diagram mermaid draws, ```dot for Graphviz's.
// Each library is loaded only when a fence of its kind first comes near the screen, so that the page's first load
// carries neither. A diagram is drawn in the page's palette, light or dark as the page is, at its own size, scrolling
// inside its box when wider than the text; one that cannot be drawn shows its source as code, saying so.
//
// What a diagram's source says must never become markup the page did not mean: mermaid draws at its strict security
// level, with no HTML in its labels, and keeps the page's look and security whatever the source's own configuration
// says; Graphviz's drawing is sanitised as SVG, with no script and no foreign object.

import { CodeIcon } from '@primer/octicons-react';
import { IconButton } from '@primer/react';
import { useEffect, useRef, useState, useSyncExternalStore, type RefObject } from 'react';
import type { MermaidConfig } from 'mermaid';
import type { Viz } from '@viz-js/viz';

import { t } from './text';

export type DiagramKind = 'mermaid' | 'dot';

/// The kind of diagram a fence's language names, if it names one.
export function diagramKind(lang: string): DiagramKind | null {
  const l = lang.toLowerCase();
  return l === 'mermaid' || l === 'dot' ? l : null;
}

const DARK = '(prefers-color-scheme: dark)';

function subscribeDark(changed: () => void): () => void {
  if (typeof window === 'undefined' || window.matchMedia === undefined) return () => {};
  const q = window.matchMedia(DARK);
  q.addEventListener('change', changed);
  return () => q.removeEventListener('change', changed);
}

/// Whether the page is dark: it follows the system's appearance, as Primer's auto mode does (main.tsx).
function useDark(): boolean {
  return useSyncExternalStore(
    subscribeDark,
    () => typeof window !== 'undefined' && window.matchMedia !== undefined && window.matchMedia(DARK).matches,
    () => false,
  );
}

/// Whether an element has come near the screen; once it has, it stays so.
function useSeen(ref: RefObject<HTMLElement | null>): boolean {
  const [seen, setSeen] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (seen || el === null) return;
    if (typeof IntersectionObserver === 'undefined') {
      setSeen(true);
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setSeen(true);
          io.disconnect();
        }
      },
      { rootMargin: '200px' },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [ref, seen]);
  return seen;
}

/// The page's palette tokens a diagram is drawn in.
const TOKENS = {
  fg: '--fgColor-default',
  fgMuted: '--fgColor-muted',
  bg: '--bgColor-default',
  bgMuted: '--bgColor-muted',
  border: '--borderColor-default',
  accent: '--borderColor-accent-emphasis',
  accentMuted: '--bgColor-accent-muted',
  success: '--borderColor-success-emphasis',
  successMuted: '--bgColor-success-muted',
  attention: '--borderColor-attention-emphasis',
  attentionMuted: '--bgColor-attention-muted',
  danger: '--fgColor-danger',
  dangerMuted: '--bgColor-danger-muted',
  onEmphasis: '--fgColor-onEmphasis',
} as const;

type Colours = Record<keyof typeof TOKENS, string>;

/// The colours of the parts a diagram colours one by one (a mind map's branches, a timeline's periods, a pie's
/// slices, a git graph's branches), in this order: Primer's display colours, each filled in its emphasis with words on
/// it as on a state's chip, which reads in either theme.
const SERIES = ['blue', 'green', 'orange', 'purple', 'pink', 'teal', 'yellow', 'coral', 'indigo', 'lime', 'cyan', 'plum'];

/// How the page looks where a diagram stands: its colours, each as `#rrggbb` or `#rrggbbaa`, which both libraries
/// read; its text's font and size, as CSS gives it and in pixels; and the width of the diagram's box.
type Look = { dark: boolean; colours: Colours; series: string[]; font: string; fontSize: string; px: number; width: number };

/// A colour as hexadecimal, from the browser's `rgb()` or `rgba()` of it.
function hex(computed: string): string {
  const parts = /\(([^)]*)\)/.exec(computed)?.[1].split(/[\s,/]+/).filter((x) => x !== '') ?? [];
  const byte = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, '0');
  const [r, g, b] = parts.slice(0, 3).map(Number);
  const a = parts[3] !== undefined ? Number.parseFloat(parts[3]) * (parts[3].endsWith('%') ? 2.55 : 255) : 255;
  return `#${byte(r)}${byte(g)}${byte(b)}${a < 255 ? byte(a) : ''}`;
}

/// The page's look as it is at an element: each token read through a probe inside it, where the page's theme holds.
function lookAt(el: HTMLElement, dark: boolean): Look {
  const probe = document.createElement('span');
  el.appendChild(probe);
  const read = (token: string) => {
    probe.style.color = `var(${token})`;
    return hex(getComputedStyle(probe).color);
  };
  const colours = {} as Colours;
  for (const [name, token] of Object.entries(TOKENS) as Array<[keyof Colours, string]>) colours[name] = read(token);
  const series = SERIES.map((c) => read(`--display-${c}-bgColor-emphasis`));
  probe.remove();
  const style = getComputedStyle(el);
  return {
    dark,
    colours,
    series,
    font: style.fontFamily,
    fontSize: style.fontSize,
    px: Number.parseFloat(style.fontSize),
    width: el.clientWidth,
  };
}

/// The narrowest a Gantt chart is drawn, which mermaid draws as wide as its box: narrower, its days cannot be read.
const GANTT_LEAST = 600;

/// The keys of mermaid's configuration a diagram's own configuration may not change: mermaid's own, and those that
/// keep it in the page's look and with no HTML in its labels.
const SECURE = [
  'secure',
  'securityLevel',
  'startOnLoad',
  'maxTextSize',
  'suppressErrorRendering',
  'maxEdges',
  'htmlLabels',
  'theme',
  'themeVariables',
  'themeCSS',
  'darkMode',
  'fontFamily',
];

function mermaidConfig(look: Look): MermaidConfig {
  const c = look.colours;
  return {
    startOnLoad: false,
    securityLevel: 'strict',
    htmlLabels: false,
    suppressErrorRendering: true,
    secure: SECURE,
    theme: 'base',
    // The kinds mermaid draws in a theme of their own by default are drawn in the page's too.
    usecase: { theme: 'base' },
    venn: { theme: 'base' },
    gantt: { useWidth: Math.max(look.width, GANTT_LEAST) },
    darkMode: look.dark,
    fontFamily: look.font,
    // A timeline draws its lines in the colour of the words on its periods, which are on a fill: drawn in the page's
    // lines' colour instead, so that they show on the page's background.
    themeCSS: `.lineWrapper line { stroke: ${look.colours.fgMuted}; }`,
    themeVariables: {
      darkMode: look.dark,
      fontFamily: look.font,
      fontSize: look.fontSize,
      background: c.bg,
      primaryColor: c.accentMuted,
      primaryBorderColor: c.accent,
      primaryTextColor: c.fg,
      nodeBorder: c.accent,
      mainBkg: c.accentMuted,
      secondaryColor: c.successMuted,
      secondaryBorderColor: c.success,
      secondaryTextColor: c.fg,
      tertiaryColor: c.bgMuted,
      tertiaryBorderColor: c.border,
      tertiaryTextColor: c.fg,
      textColor: c.fg,
      titleColor: c.fg,
      lineColor: c.fgMuted,
      clusterBkg: c.bgMuted,
      clusterBorder: c.border,
      edgeLabelBackground: c.bg,
      noteBkgColor: c.attentionMuted,
      noteBorderColor: c.attention,
      noteTextColor: c.fg,
      errorBkgColor: c.dangerMuted,
      errorTextColor: c.danger,
      altSectionBkgColor: c.bg,
      sectionBkgColor: c.bgMuted,
      sectionBkgColor2: c.bgMuted,
      excludeBkgColor: c.bgMuted,
      gridColor: c.border,
      taskBkgColor: c.accentMuted,
      taskBorderColor: c.accent,
      activeTaskBkgColor: c.successMuted,
      activeTaskBorderColor: c.success,
      doneTaskBkgColor: c.bgMuted,
      doneTaskBorderColor: c.border,
      critBkgColor: c.dangerMuted,
      critBorderColor: c.danger,
      todayLineColor: c.danger,
      vertLineColor: c.accent,
      pieStrokeColor: c.bg,
      pieOuterStrokeColor: c.border,
      pieSectionTextColor: c.onEmphasis,
      ...Object.fromEntries(
        look.series.flatMap((x, i) => [
          [`cScale${i}`, x],
          [`cScalePeer${i}`, x],
          [`cScaleInv${i}`, x],
          [`cScaleLabel${i}`, c.onEmphasis],
          [`pie${i + 1}`, x],
          [`git${i}`, x],
          [`gitBranchLabel${i}`, c.onEmphasis],
        ]),
      ),
    },
  };
}

/// Mermaid's drawings, one after another: its configuration is the module's, set for each drawing.
let mermaidQueue: Promise<unknown> = Promise.resolve();
let drawings = 0;

async function drawMermaid(source: string, look: Look): Promise<string> {
  const { default: mermaid } = await import('mermaid');
  drawings += 1;
  const id = `wf${drawings}d`;
  const run = mermaidQueue.then(() => {
    mermaid.initialize(mermaidConfig(look));
    return mermaid.render(id, source);
  });
  mermaidQueue = run.catch(() => undefined);
  const { svg } = await run;
  return svg.replaceAll(id, ID);
}

let viz: Promise<Viz> | null = null;

async function drawDot(source: string, look: Look): Promise<string> {
  const [{ instance }, { default: purify }] = await Promise.all([import('@viz-js/viz'), import('dompurify')]);
  viz ??= instance();
  const c = look.colours;
  // Helvetica, which Graphviz measures text in as Arial, for a sans-serif face: it cannot measure the page's. What the
  // source sets itself stands; a node it fills takes the page's tint.
  const font = { fontname: 'Helvetica', fontsize: look.px, fontcolor: c.fg };
  const drawn = (await viz).render(source, {
    format: 'svg',
    graphAttributes: { ...font, bgcolor: 'transparent', color: c.border },
    nodeAttributes: { ...font, color: c.accent, fillcolor: c.accentMuted },
    edgeAttributes: { ...font, color: c.fgMuted },
  });
  if (drawn.status !== 'success') throw new Error(drawn.errors.map((e) => e.message).join('\n'));
  const svg = drawn.output.slice(drawn.output.indexOf('<svg'));
  return purify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    FORBID_TAGS: ['script', 'foreignObject'],
  });
}

/// The id a drawing's markup is cached under, each one drawn taking an id of its own in its place.
const ID = 'wf-diagram-id';

/// A drawing at its own size, in pixels of its own units: a library that scales it to the width of its box is undone,
/// so that a wide diagram scrolls in its box rather than shrinking past reading.
function atOwnSize(svg: string): string {
  const tpl = document.createElement('template');
  tpl.innerHTML = svg;
  const el = tpl.content.querySelector('svg');
  const box = el?.getAttribute('viewBox')?.trim().split(/[\s,]+/).map(Number);
  if (el == null || box === undefined || box.length !== 4 || box.some((n) => !Number.isFinite(n))) {
    throw new Error('no drawing');
  }
  el.setAttribute('width', String(box[2]));
  el.setAttribute('height', String(box[3]));
  el.style.removeProperty('max-width');
  return tpl.innerHTML;
}

/// The drawings made, by kind, theme, box width for mermaid (which draws a Gantt chart as wide as its box) and source,
/// the latest last: a diagram shown again is not drawn again.
const drawn = new Map<string, Promise<string>>();
const KEPT = 64;

function draw(kind: DiagramKind, source: string, look: Look): Promise<string> {
  const key = `${kind}\n${look.dark ? 'dark' : 'light'}\n${kind === 'mermaid' ? look.width : ''}\n${source}`;
  let svg = drawn.get(key);
  if (svg === undefined) {
    svg = (kind === 'mermaid' ? drawMermaid(source, look) : drawDot(source, look)).then(atOwnSize);
    svg.catch(() => drawn.delete(key));
  } else {
    drawn.delete(key);
  }
  drawn.set(key, svg);
  if (drawn.size > KEPT) drawn.delete(drawn.keys().next().value as string);
  return svg;
}

let shown = 0;

/// A diagram, drawn once it comes near the screen and again when the page's theme changes; its source as code while
/// it is not drawn yet, and with a note under it if it cannot be. Under the pointer, a button in its corner shows its
/// source in its place, and the diagram again.
export function Diagram({ kind, source }: { kind: DiagramKind; source: string }) {
  const box = useRef<HTMLDivElement>(null);
  const seen = useSeen(box);
  const dark = useDark();
  const [id] = useState(() => `wf-diagram-${(shown += 1)}`);
  const [got, setGot] = useState<{ svg: string } | { failed: true } | null>(null);
  const [asCode, setAsCode] = useState(false);
  useEffect(() => {
    const el = box.current;
    if (!seen || el === null) return;
    let live = true;
    draw(kind, source, lookAt(el, dark)).then(
      (svg) => {
        if (live) setGot({ svg: svg.replaceAll(ID, id) });
      },
      () => {
        if (live) setGot({ failed: true });
      },
    );
    return () => {
      live = false;
    };
  }, [seen, kind, source, dark, id]);
  const svg = got !== null && 'svg' in got ? got.svg : null;
  return (
    <div ref={box} className="diagram">
      {svg !== null && !asCode ? (
        <div className="diagram-drawing" dangerouslySetInnerHTML={{ __html: svg }} />
      ) : (
        <pre className="code-block">{source}</pre>
      )}
      {got !== null && 'failed' in got ? <p className="diagram-note">{t('diagram_failed')}</p> : null}
      {svg !== null ? (
        <IconButton
          className="diagram-toggle"
          icon={CodeIcon}
          size="small"
          variant="invisible"
          aria-label={asCode ? t('diagram_drawing') : t('diagram_source')}
          aria-pressed={asCode}
          onClick={() => setAsCode(!asCode)}
        />
      ) : null}
    </div>
  );
}
