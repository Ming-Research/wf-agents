// A diagram written in a passage as a fence of its kind (the design of the owner's page, 9 to 13): ```mermaid for any
// diagram mermaid draws, ```dot for Graphviz's. Each library is loaded only when a fence of its kind first comes near
// the screen, so that a page that shows none does not pay for it. A diagram is drawn in the page's colours, light or
// dark and in the accent as the page is, once, at its own size, and shown at that size or, when wider than its place,
// as narrow as the place, never cut at the right and never scrolling sideways; then it, or a button in its corner,
// opens it larger in a window of its own, in the viewer that moves and scales a flowchart. One that cannot be drawn
// shows its source with a note under it; one that is drawn can show its source in its place.
//
// What a diagram's source says never becomes markup the page did not mean (principle agent-writing-is-data): mermaid
// draws at its strictest setting, with no markup in its labels, and keeps the page's look and setting whatever the
// source's own configuration says; and every drawing is cleaned as SVG, with no script and no foreign object. The
// larger view shows that same cleaned markup, never a drawing made again from the source. A drawing stands in a shadow
// root of its own.

import { type MouseEvent, type PointerEvent, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import type { MermaidConfig } from 'mermaid';
import type { Viz } from '@viz-js/viz';

import { onAccent } from './accent';
import { DRAG_LIMIT, fitView, seenOf, Viewer, type ViewerHandle } from './graph/Viewer';

export type DiagramKind = 'mermaid' | 'dot';

/// The kind of diagram a fence's language names, if it names one.
export function diagramKind(lang: string): DiagramKind | null {
  const l = lang.toLowerCase();
  return l === 'mermaid' || l === 'dot' ? l : null;
}

const DARK = '(prefers-color-scheme: dark)';

/// Whether the page is dark: as the bar's switch has it, or as the system is where the switch follows it (`theme.ts`).
function isDark(): boolean {
  const theme = document.documentElement.getAttribute('data-theme');
  if (theme === 'dark' || theme === 'light') return theme === 'dark';
  return typeof window.matchMedia === 'function' && window.matchMedia(DARK).matches;
}

/// How many times the page's colours have changed since the diagram was shown: light or dark, by the bar's switch or
/// the system's, and the accent the owner picks.
function useLookChanges(): number {
  const [changes, setChanges] = useState(0);
  useEffect(() => {
    const changed = () => setChanges((n) => n + 1);
    const media = typeof window.matchMedia === 'function' ? window.matchMedia(DARK) : null;
    media?.addEventListener('change', changed);
    const root = new MutationObserver(changed);
    root.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    const accent = onAccent(changed);
    return () => {
      media?.removeEventListener('change', changed);
      root.disconnect();
      accent();
    };
  }, []);
  return changes;
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

/// The page's colours a diagram is drawn in.
const TOKENS = {
  fg: '--text',
  fgMuted: '--muted',
  bg: '--card',
  bgMuted: '--ground',
  border: '--line',
  accent: '--accent',
  accentMuted: '--accent-tint',
  success: '--green',
  successMuted: '--green-tint',
  attention: '--orange',
  attentionMuted: '--orange-tint',
  danger: '--red-text',
  dangerMuted: '--red-tint',
  onColour: '--on-colour',
} as const;

type Colours = Record<keyof typeof TOKENS, string>;

/// The colours of the parts a diagram colours one by one (a mind map's branches, a timeline's periods, a pie's slices,
/// a git graph's branches), in this order: the page's colours that tell things apart, as the lines of the flow are.
const SERIES = ['--flow-line-1', '--flow-line-2', '--flow-line-3', '--flow-line-4', '--flow-line-5'];

/// How the page looks where a diagram stands: its colours, each as `#rrggbb` or `#rrggbbaa`, which both libraries
/// read; the colours of the parts coloured one by one, each with the colour of the words on it; its text's font and
/// size, as CSS gives it and in pixels; and the width of the diagram's place.
type Look = {
  dark: boolean;
  colours: Colours;
  series: { fill: string; words: string }[];
  font: string;
  fontSize: string;
  px: number;
  width: number;
};

/// A colour as hexadecimal, from the browser's `rgb()`, `rgba()` or `color(srgb …)` of it.
function hex(computed: string): string {
  const srgb = computed.startsWith('color(');
  const parts =
    /\(([^)]*)\)/
      .exec(computed)?.[1]
      .replace(/^srgb\s+/, '')
      .split(/[\s,/]+/)
      .filter((x) => x !== '') ?? [];
  const byte = (n: number) =>
    Math.round(Math.min(255, Math.max(0, n)))
      .toString(16)
      .padStart(2, '0');
  const [r, g, b] = parts.slice(0, 3).map((x) => Number(x) * (srgb ? 255 : 1));
  const alpha = parts[3];
  const a =
    alpha === undefined ? 255 : alpha.endsWith('%') ? Number.parseFloat(alpha) * 2.55 : Number.parseFloat(alpha) * 255;
  return `#${byte(r)}${byte(g)}${byte(b)}${a < 255 ? byte(a) : ''}`;
}

/// How light a colour is to the eye, from 0 for black to 1 for white, as the contrast of text is measured.
function lightness(colour: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = Number.parseInt(colour.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/// How far apart two colours stand for words of one on the other.
function contrast(a: string, b: string): number {
  const [x, y] = [lightness(a), lightness(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/// The page's look as it is at an element: each colour read through a probe inside it, where the page's theme holds;
/// the words on a part coloured one by one in the text's colour or the white on a colour, whichever stands further
/// from it.
function lookAt(el: HTMLElement): Look {
  const probe = document.createElement('span');
  el.appendChild(probe);
  const read = (token: string) => {
    probe.style.color = `var(${token})`;
    return hex(getComputedStyle(probe).color);
  };
  const colours = {} as Colours;
  for (const [name, token] of Object.entries(TOKENS) as [keyof Colours, string][]) colours[name] = read(token);
  const series = SERIES.map((token) => {
    const fill = read(token);
    const words = contrast(fill, colours.fg) >= contrast(fill, colours.onColour) ? colours.fg : colours.onColour;
    return { fill, words };
  });
  probe.remove();
  const style = getComputedStyle(el);
  return {
    dark: isDark(),
    colours,
    series,
    font: style.fontFamily,
    fontSize: style.fontSize,
    px: Number.parseFloat(style.fontSize),
    width: el.clientWidth,
  };
}

/// The narrowest a Gantt chart is drawn, which mermaid draws as wide as its place: narrower, its days cannot be read.
const GANTT_LEAST = 600;

/// The keys of mermaid's configuration a diagram's own configuration may not change: mermaid's own, and those that
/// keep it in the page's look and with no markup in its labels.
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
    gantt: { useWidth: Math.max(look.width, GANTT_LEAST) },
    darkMode: look.dark,
    fontFamily: look.font,
    // A timeline draws its lines in the colour of the words on its periods, which are on a fill: drawn in the page's
    // lines' colour instead, so that they show on the page's ground.
    themeCSS: `.lineWrapper line { stroke: ${c.fgMuted}; }`,
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
      ...Object.fromEntries(
        look.series.flatMap((x, i) => [
          [`cScale${i}`, x.fill],
          [`cScalePeer${i}`, x.fill],
          [`cScaleInv${i}`, x.fill],
          [`cScaleLabel${i}`, x.words],
          [`pie${i + 1}`, x.fill],
          [`git${i}`, x.fill],
          [`gitBranchLabel${i}`, x.words],
        ]),
      ),
    },
  };
}

/// A drawing cleaned as SVG: no script, no foreign object, nothing but what an SVG draws with.
async function cleaned(svg: string): Promise<string> {
  const { default: purify } = await import('dompurify');
  return purify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    FORBID_TAGS: ['script', 'foreignObject'],
  });
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
  return cleaned(svg.replaceAll(id, ID));
}

let viz: Promise<Viz> | null = null;

async function drawDot(source: string, look: Look): Promise<string> {
  const { instance } = await import('@viz-js/viz');
  viz ??= instance();
  const c = look.colours;
  // Helvetica, which Graphviz measures text in as Arial, for a sans-serif face: it cannot measure the page's. What the
  // source sets itself stands; a node it fills takes the accent's tint.
  const font = { fontname: 'Helvetica', fontsize: look.px, fontcolor: c.fg };
  const drawn = (await viz).render(source, {
    format: 'svg',
    graphAttributes: { ...font, bgcolor: 'transparent', color: c.border },
    nodeAttributes: { ...font, color: c.accent, fillcolor: c.accentMuted },
    edgeAttributes: { ...font, color: c.fgMuted },
  });
  if (drawn.status !== 'success') throw new Error(drawn.errors.map((e) => e.message).join('\n'));
  return cleaned(drawn.output.slice(drawn.output.indexOf('<svg')));
}

/// The id a drawing's markup is kept under, each diagram drawn taking an id of its own in its place.
const ID = 'wf-diagram-id';

/// A drawing: its markup, whose ids are `ID` until a diagram takes it as its own, and its size in pixels of its own
/// units, which is also the size its width and height attributes give it.
type Drawn = { svg: string; width: number; height: number };

/// A drawing at its own size, in pixels of its own units: a library that scales it to the width of its place is
/// undone, so that a diagram is shown at the size it was drawn and scaled from that, whatever place it stands in.
function atOwnSize(svg: string): Drawn {
  const tpl = document.createElement('template');
  tpl.innerHTML = svg;
  const el = tpl.content.querySelector('svg');
  const box = el
    ?.getAttribute('viewBox')
    ?.trim()
    .split(/[\s,]+/)
    .map(Number);
  if (el == null || box === undefined || box.length !== 4 || box.some((n) => !Number.isFinite(n))) {
    throw new Error('no drawing');
  }
  el.setAttribute('width', String(box[2]));
  el.setAttribute('height', String(box[3]));
  el.style.removeProperty('max-width');
  return { svg: tpl.innerHTML, width: box[2], height: box[3] };
}

/// A drawing's markup under an id of its own, so that two copies of one drawing in a page (the diagram's and its
/// larger view) do not meet in the styles and arrow marks that mermaid scopes by id.
function under(drawn: Drawn, id: string): string {
  return drawn.svg.replaceAll(ID, id);
}

/// The drawings made, by kind, colours, width of the place for mermaid (which draws a Gantt chart as wide as its
/// place) and source, the latest last: a diagram shown again is not drawn again.
const made = new Map<string, Promise<Drawn>>();
const KEPT = 64;

function draw(kind: DiagramKind, source: string, look: Look): Promise<Drawn> {
  const key = [kind, JSON.stringify([look.colours, look.series]), kind === 'mermaid' ? look.width : '', source].join(
    '\n',
  );
  let drawing = made.get(key);
  if (drawing === undefined) {
    drawing = (kind === 'mermaid' ? drawMermaid(source, look) : drawDot(source, look)).then(atOwnSize);
    drawing.catch(() => made.delete(key));
  } else {
    made.delete(key);
  }
  made.set(key, drawing);
  if (made.size > KEPT) made.delete(made.keys().next().value as string);
  return drawing;
}

/// Whether a drawing of `natural` pixels is shown narrower than it was drawn, in a place of `box` pixels: the place
/// has been measured and is the narrower. A drawing that fits is never made larger.
function isScaled(natural: number, box: number): boolean {
  return box > 0 && box < natural;
}

/// The width of an element in pixels, kept as it changes; 0 until it has been measured.
function useWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (el === null || typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => setWidth(el.clientWidth));
    watch.observe(el);
    return () => watch.disconnect();
  }, [ref]);
  return width;
}

/// Whether a press at `down` and a release at `up` (in the window's pixels) make a click on the drawing: the pointer
/// moved no further than the viewer lets a click move, so that dragging across the diagram to select its words does
/// not open it.
function isClick(down: { x: number; y: number }, up: { x: number; y: number }): boolean {
  return Math.hypot(up.x - down.x, up.y - down.y) <= DRAG_LIMIT;
}

/// A drawing's markup in a shadow root of its own, with `style` the only style that reaches it: none of the page's
/// styles, whose names a drawing's own classes share (mermaid's `label`, `edge` and `legend` among them), changes it.
function Drawing({
  svg,
  style,
  className,
  onPointerDown,
  onClick,
}: {
  svg: string;
  style: string;
  className?: string;
  onPointerDown?: (e: PointerEvent<HTMLDivElement>) => void;
  onClick?: (e: MouseEvent<HTMLDivElement>) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const root = el.shadowRoot ?? el.attachShadow({ mode: 'open' });
    root.innerHTML = `<style>${style}</style>${svg}`;
  }, [svg, style]);
  return <div ref={host} className={className} onPointerDown={onPointerDown} onClick={onClick} />;
}

/// A drawing in a passage: as wide as it was drawn, or as its place where that is narrower.
const IN_PLACE = 'svg { display: block; max-width: 100%; height: auto; }';
/// A drawing in the viewer, which moves and scales it: at the size it was drawn.
const IN_VIEWER = 'svg { display: block; }';

/// The inside of the larger view: the drawing, in the markup the diagram has, under an id of its own, in the viewer a
/// flowchart is moved and scaled in, shown whole once its place has a size.
function Enlarged({ drawn, id }: { drawn: Drawn; id: string }) {
  const stage = useRef<HTMLDivElement>(null);
  const viewer = useRef<ViewerHandle>(null);
  useEffect(() => {
    const el = stage.current;
    if (el === null) return;
    let placed = false;
    const place = () => {
      if (placed || el.clientWidth === 0 || el.clientHeight === 0) return;
      placed = true;
      viewer.current?.to(fitView(seenOf({ width: el.clientWidth, height: el.clientHeight }), drawn), false);
    };
    place();
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(place);
    watch.observe(el);
    return () => watch.disconnect();
  }, [drawn]);
  return (
    <div ref={stage} className="diagram-window-stage">
      <Viewer width={drawn.width} height={drawn.height} ref={viewer}>
        <Drawing svg={under(drawn, `${id}-larger`)} style={IN_VIEWER} />
      </Viewer>
    </div>
  );
}

/// The window a diagram opens larger in, over the page and nearly as large as it: its name, a button that closes it,
/// the drawing in the viewer, and how the viewer is moved and scaled. Escape, or a press beside it, closes it too.
function DiagramWindow({ drawn, id, onClose }: { drawn: Drawn; id: string; onClose: () => void }) {
  const box = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = box.current;
    if (dialog && !dialog.open) dialog.showModal();
  }, []);
  return (
    <dialog
      ref={box}
      className="diagram-window"
      aria-label="Diagram"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => {
        if (e.target === box.current) onClose();
      }}
    >
      <div className="diagram-window-head">
        <span className="diagram-window-title">Diagram</span>
        <button type="button" className="link" onClick={onClose}>
          Close
        </button>
      </div>
      <Enlarged drawn={drawn} id={id} />
      <p className="diagram-window-hint">
        Two fingers or drag: move. Pinch, or Shift with two fingers: zoom. Space: the whole diagram, and back.
      </p>
    </dialog>
  );
}

let shown = 0;

/// A diagram, drawn once it comes near the screen and again when the page's colours change; its source as code while
/// it is not drawn yet, and with a note under it if it cannot be. It is made as narrow as its place when wider, and
/// then it, or a button in its corner, opens it larger. A button in its corner shows its source in its place, and the
/// diagram again.
export function Diagram({ kind, source }: { kind: DiagramKind; source: string }) {
  const box = useRef<HTMLDivElement>(null);
  const seen = useSeen(box);
  const changes = useLookChanges();
  const width = useWidth(box);
  const [id] = useState(() => `wf-diagram-${(shown += 1)}`);
  const [got, setGot] = useState<{ drawn: Drawn } | { failed: true } | null>(null);
  const [asCode, setAsCode] = useState(false);
  const [larger, setLarger] = useState(false);
  // Where the pointer last went down on the drawing, to tell a click from a drag.
  const pressed = useRef<{ x: number; y: number } | null>(null);
  useEffect(() => {
    const el = box.current;
    if (!seen || el === null) return;
    let live = true;
    draw(kind, source, lookAt(el)).then(
      (drawn) => {
        if (live) setGot({ drawn });
      },
      () => {
        if (live) setGot({ failed: true });
      },
    );
    return () => {
      live = false;
    };
  }, [seen, kind, source, changes]);
  const drawn = got !== null && 'drawn' in got ? got.drawn : null;
  const svg = useMemo(() => (drawn !== null ? under(drawn, id) : null), [drawn, id]);
  const scaled = drawn !== null && isScaled(drawn.width, width);
  return (
    <div ref={box} className="diagram">
      {svg !== null && !asCode ? (
        <Drawing
          svg={svg}
          style={IN_PLACE}
          className={scaled ? 'diagram-drawing diagram-drawing-scaled' : 'diagram-drawing'}
          onPointerDown={(e) => {
            pressed.current = { x: e.clientX, y: e.clientY };
          }}
          onClick={
            scaled
              ? (e) => {
                  const down = pressed.current;
                  if (down !== null && isClick(down, { x: e.clientX, y: e.clientY })) setLarger(true);
                }
              : undefined
          }
        />
      ) : (
        <pre className="content">
          <code>{source}</code>
        </pre>
      )}
      {got !== null && 'failed' in got && (
        <p className="diagram-note">This diagram could not be drawn; its source is above.</p>
      )}
      {drawn !== null && (
        <div className="diagram-tools">
          {scaled && !asCode && (
            <button type="button" className="link diagram-tool" onClick={() => setLarger(true)}>
              Enlarge
            </button>
          )}
          <button type="button" className="link diagram-tool" aria-pressed={asCode} onClick={() => setAsCode(!asCode)}>
            {asCode ? 'Diagram' : 'Source'}
          </button>
        </div>
      )}
      {larger && drawn !== null && <DiagramWindow drawn={drawn} id={id} onClose={() => setLarger(false)} />}
    </div>
  );
}
