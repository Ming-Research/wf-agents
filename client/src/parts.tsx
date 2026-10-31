// The parts the page shows like things with, each with one look wherever the page shows it, as the UI spec maps them
// to Primer: a round, by its name; a round at a cell; who wrote something (who to whom), its round at its cell, and
// when; what an agent wrote, as a block, its head and who wrote it over its text; a section under its heading and how
// many it holds, folded or open, and a sub-heading inside it; an item, by its number and title; a pull request, by its
// number; the words the owner writes beside the button that sends them, with the images that go with them; the images
// the owner sent, as thumbnails; and the form that answers or says something, its words and the button that sends them.

import { ChevronDownIcon, ChevronRightIcon, PaperclipIcon, XIcon } from '@primer/octicons-react';
import {
  Button,
  CounterLabel,
  Details,
  Flash,
  Heading,
  IconButton,
  Label,
  type LabelColorOptions,
  Link as PrimerLink,
  Textarea,
} from '@primer/react';
import {
  createContext,
  createElement,
  type Dispatch,
  Fragment,
  type KeyboardEvent,
  type ReactNode,
  type SetStateAction,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';

import { roundName, time } from './format';
import { admit, IMAGE_KINDS, imageHref, IMAGES_MOST, megabytes, pastedFiles, type PendingImage, readImage } from './images';
import { useStatus } from './live';
import { Link, workHref } from './route';
import { t } from './text';

/// What opens a round of a work where the page shows it in place.
export type OpenRound = (work: number, round: number) => void;

/// A round, by its name: a click opens it where the page shows it (`onRound`), else on its work's page.
export function RoundLink({ work, round, onRound }: { work: number; round: number; onRound?: OpenRound }) {
  if (onRound === undefined) {
    return (
      <Link to={workHref(work, { round })} className="round-link">
        {roundName(work, round)}
      </Link>
    );
  }
  return (
    <PrimerLink
      as="button"
      type="button"
      className="round-link"
      onClick={(e: { stopPropagation: () => void }) => {
        e.stopPropagation();
        onRound(work, round);
      }}
    >
      {roundName(work, round)}
    </PrimerLink>
  );
}

/// The one way the page writes a round at a cell, wherever it names the two together: the round's name first, then the
/// cell's code and title in parentheses, "R10-55 (Q0 What can be picked)"; the round alone where its cell is not known,
/// and the cell alone where no round has run there yet. Its parts as given, so that a tip's text and the page's part
/// ([`RoundAt`]) are written alike.
function roundAt<T>(round: T | null, cell: T | null): (T | string)[] {
  if (round === null) return cell === null ? [] : [cell];
  return cell === null ? [round] : [round, ' (', cell, ')'];
}

/// A round at a cell as plain text, for a tip: "R10-55 (Q0 What can be picked)".
export function roundAtText(round: string | null, cell: string | null | undefined, title?: string | null): string {
  const named = cell !== undefined && cell !== null && cell !== '' ? (title ? `${cell} ${title}` : cell) : null;
  return roundAt(round, named).join('');
}

/// A round at a cell on the page, as [`roundAt`] writes it: `round` as the place shows it, its link or, in a panel's
/// title, its name; the cell by its code and title, a link that opens it where `onCell` is given, saying `hint` on
/// hovering.
export function RoundAt({ round, cell, title, onCell, hint }: {
  round: ReactNode;
  cell: string | null | undefined;
  title?: string | null;
  onCell?: () => void;
  hint?: string;
}) {
  const named =
    cell !== undefined && cell !== null && cell !== '' ? (
      <>
        <span className="cell-id">{cell}</span>
        {title ? ` ${title}` : null}
      </>
    ) : null;
  const shown =
    named !== null && onCell !== undefined ? (
      <LinkButton title={hint} onClick={onCell}>
        {named}
      </LinkButton>
    ) : (
      named
    );
  return (
    <span className="round-at">
      {roundAt(round ?? null, shown).map((x, i) => (
        <Fragment key={i}>{x}</Fragment>
      ))}
    </span>
  );
}

/// The titles of the cells of the graph a page shows, by their codes, which [`By`] gives beside a code.
export const CellTitles = createContext<ReadonlyMap<string, string>>(new Map());

/// Who wrote something: what it was first, when there is something to say (`children`); who to whom in a
/// conversation; the round at its cell, the cell as the flow names it, by its code and title, and the round opening as
/// a round link does; and when.
export function By({ who, cell, at, work, round, onRound, children }: {
  who?: ReactNode;
  cell?: string | null;
  at?: number | null;
  work?: number | null;
  round?: number | null;
  onRound?: OpenRound;
  children?: ReactNode;
}) {
  const titles = useContext(CellTitles);
  const title = cell !== undefined && cell !== null ? titles.get(cell) : undefined;
  const ran = work !== undefined && work !== null && round !== undefined && round !== null;
  return (
    <span className="by">
      {children}
      {who !== undefined ? <span className="by-who">{who}</span> : null}
      {ran || (cell !== undefined && cell !== null && cell !== '') ? (
        <RoundAt round={ran ? <RoundLink work={work} round={round} onRound={onRound} /> : null} cell={cell} title={title} />
      ) : null}
      {at !== undefined && at !== null ? <span>{time(at)}</span> : null}
    </span>
  );
}

/// What an agent wrote, or a thing of the same kind, as a block: its head (what it is, as chips and a title) and who
/// wrote it, over its text. `tone` sets it apart: new, changed or gone since before.
export function Block({ id, head, by, tone, children }: {
  id?: string;
  head?: ReactNode;
  by?: ReactNode;
  tone?: string;
  children?: ReactNode;
}) {
  return (
    <div className={tone !== undefined ? `block ${tone}` : 'block'} id={id}>
      {head !== undefined || by !== undefined ? (
        <div className="block-h">
          {head}
          {by}
        </div>
      ) : null}
      {children !== undefined && children !== null && children !== false ? (
        <div className="block-body">{children}</div>
      ) : null}
    </div>
  );
}

/// The level the next heading takes where it stands: a page converted to the UI spec gives its sections 2, and each
/// section gives what it holds one more. Elsewhere a section's heading is a sub-heading.
export const HeadingLevel = createContext(3);

/// A sub-heading: the name of a part inside a section, at the level it stands at, in the sub-heading's step.
export function SubHeading({ children, className }: { children: ReactNode; className?: string }) {
  const level = Math.min(useContext(HeadingLevel), 6);
  return createElement(`h${level}`, { className: className !== undefined ? `sub-heading ${className}` : 'sub-heading' }, children);
}

/// A section: its heading, how many it holds (`n`) and who wrote it on the line that folds it, over what it holds,
/// folded unless `open`; none when it holds nothing. A section at level 2 is a page's own, its heading in the section
/// step, a line over it parting it from what comes before; one inside another is a sub-heading.
export function Section({ title, n, open = false, by, id, children }: {
  title: ReactNode;
  n?: number;
  open?: boolean;
  by?: ReactNode;
  id?: string;
  children: ReactNode;
}) {
  const level = Math.min(useContext(HeadingLevel), 6);
  if (n === 0) return null;
  return (
    <Details className="section" data-level={level} open={open} id={id}>
      <summary>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        {level === 2 ? (
          <Heading as="h2" variant="small" className="section-title">
            {title}
          </Heading>
        ) : (
          createElement(`h${level}`, { className: 'section-title sub-heading' }, title)
        )}
        {n !== undefined ? <CounterLabel>{n}</CounterLabel> : null}
        {by}
      </summary>
      <div className="section-body">
        <HeadingLevel.Provider value={level + 1}>{children}</HeadingLevel.Provider>
      </div>
    </Details>
  );
}

/// A small fold inside what a section holds (what an input holds, a long condition, earlier text): its chevron and
/// what it is on one line, what it holds under it, set in by the chevron's width; folded unless `open`. `onToggle`
/// hears it open and fold, for what is read only once it is open.
export function Fold({ summary, open = false, className, onToggle, children }: {
  summary: ReactNode;
  open?: boolean;
  className?: string;
  onToggle?: (open: boolean) => void;
  children: ReactNode;
}) {
  return (
    <details
      className={className !== undefined ? `fold ${className}` : 'fold'}
      open={open}
      onToggle={onToggle !== undefined ? (e) => onToggle(e.currentTarget.open) : undefined}
    >
      <summary>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        <span className="fold-summary">{summary}</span>
      </summary>
      <div className="fold-body">{children}</div>
    </details>
  );
}

/// The period a view counts over, kept in the address: what it is counted over, then each period the view offers as a
/// link, the one counted over in the text's colour and semibold; `className` sets it in the view.
export function PeriodSwitch({ periods, className }: {
  periods: Array<{ key: string; label: string; to: string; on: boolean }>;
  className?: string;
}) {
  return (
    <div className={className !== undefined ? `period-switch ${className}` : 'period-switch'}>
      <span className="muted">{t('sys_period')}</span>
      {periods.map((p) => (
        <Link key={p.key} to={p.to} className={p.on ? 'period-on' : undefined}>
          {p.label}
        </Link>
      ))}
    </div>
  );
}

/// An action inside a line of text that opens something on the page, drawn as Primer's Link.
export function LinkButton({ onClick, title, className, children }: {
  onClick: () => void;
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <PrimerLink
      as="button"
      type="button"
      className={className !== undefined ? `link-button ${className}` : 'link-button'}
      title={title}
      onClick={(e: { stopPropagation: () => void }) => {
        e.stopPropagation();
        onClick();
      }}
    >
      {children}
    </PrimerLink>
  );
}

/// A state or a kind that leads where the owner acts on it, such as what a work waits for: its Label inside a link to
/// another place (`to`), or inside a button that opens it on this page (`onClick`).
export function LabelLink({ to, onClick, variant, title, children }: {
  to?: string;
  onClick?: () => void;
  variant: LabelColorOptions;
  title?: string;
  children: ReactNode;
}) {
  if (to === undefined) {
    return (
      <button type="button" className="label-link label-button" title={title} onClick={onClick}>
        <Label variant={variant}>{children}</Label>
      </button>
    );
  }
  return (
    <Link to={to} className="label-link" title={title}>
      <Label variant={variant}>{children}</Label>
    </Link>
  );
}

/// An item, by its number and its title, leading to its page; `hint` says more on hovering.
export function ItemRef({ id, title, hint }: { id: number; title?: string | null; hint?: string }) {
  return (
    <Link to={`/items/${id}`} className="item-ref" title={hint}>
      <span className="item-ref-n">#{id}</span>
      {title !== undefined && title !== null && title !== '' ? ` ${title}` : null}
    </Link>
  );
}

/// A pull request of a project, by its number, opening it on GitHub; its number alone while the project's repository is
/// not known.
export function PrLink({ project, n }: { project: string | null; n: number }) {
  const status = useStatus();
  const repo = project !== null ? status?.repos[project] : undefined;
  if (repo === undefined) return <>{t('work_pr', { n })}</>;
  return (
    <PrimerLink href={`https://github.com/${repo}/pull/${n}`} target="_blank" rel="noreferrer" title={t('work_pr_hint')}>
      {t('work_pr', { n })}
    </PrimerLink>
  );
}

/// The words the owner writes beside the button that sends them, in every box that says or answers something, and the
/// images that go with them: one line at rest, as tall as the button (`small` beside a small one), growing with what is
/// written up to twelve lines, past which they scroll, with no handle to drag, the growing taking its place. Beside the
/// words a button picks images; an image pasted into the words is added, and pasted text still pastes as text. The
/// images added wait over the words, each a thumbnail with a button that takes it out, with how many against the 10 a
/// message carries; what the box refuses (a file that is no PNG, JPEG, GIF or WebP image, one over 5 MB, one past the
/// tenth) is said under them, until the box takes or loses an image or is emptied. The words and the images go together
/// when sent, and may be images alone.
export function SayBox({ text, set, images, setImages, small = false, placeholder, label, title, disabled, onKeyDown }: {
  text: string;
  set: (text: string) => void;
  images: PendingImage[];
  setImages: Dispatch<SetStateAction<PendingImage[]>>;
  small?: boolean;
  placeholder: string;
  label?: string;
  title?: string;
  disabled: boolean;
  onKeyDown: (e: KeyboardEvent<HTMLTextAreaElement>) => void;
}) {
  const box = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const [refused, setRefused] = useState<string | null>(null);
  // Sent, the box is emptied, and what it refused before goes with what it held.
  const [held, setHeld] = useState(images);
  if (held !== images) {
    setHeld(images);
    if (images.length === 0 && text === '') setRefused(null);
  }
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [text]);
  const add = (files: File[]) => {
    const { take, refused: no } = admit(images.length, files);
    setRefused(no.length > 0 ? no.join(' ') : null);
    Promise.all(take.map(readImage)).then(
      (read) => setImages((now) => [...now, ...read].slice(0, IMAGES_MOST)),
      (x: unknown) => setRefused(x instanceof Error ? x.message : String(x)),
    );
  };
  const size = small ? 'small' : 'medium';
  return (
    <div className="say-box">
      {images.length > 0 ? (
        <div className="say-images" role="group" aria-label={t('say_images')}>
          {images.map((im) => (
            <span key={im.key} className="say-image">
              <img src={im.data_url} alt={im.name} title={t('say_image_hint', { name: im.name, size: megabytes(im.size) })} />
              <IconButton
                icon={XIcon}
                size="small"
                variant="invisible"
                className="say-image-remove"
                aria-label={t('say_image_remove', { name: im.name })}
                disabled={disabled}
                onClick={() => {
                  setRefused(null);
                  setImages((now) => now.filter((x) => x.key !== im.key));
                }}
              />
            </span>
          ))}
          <span className="say-images-count">{t('say_images_count', { n: images.length, most: IMAGES_MOST })}</span>
        </div>
      ) : null}
      {refused !== null ? <p className="say-refused">{refused}</p> : null}
      <div className="say-box-row">
        <IconButton
          icon={PaperclipIcon}
          size={size}
          aria-label={t('say_add_images')}
          disabled={disabled || images.length >= IMAGES_MOST}
          onClick={() => picker.current?.click()}
        />
        <input
          ref={picker}
          type="file"
          multiple
          hidden
          accept={IMAGE_KINDS.join(',')}
          onChange={(e) => {
            add([...(e.target.files ?? [])]);
            e.target.value = '';
          }}
        />
        <Textarea
          ref={box}
          block
          resize="none"
          rows={1}
          className={small ? 'say-box-words say-box-words-small' : 'say-box-words'}
          value={text}
          disabled={disabled}
          placeholder={placeholder}
          aria-label={label}
          title={title}
          onChange={(e) => set(e.target.value)}
          onKeyDown={onKeyDown}
          onPaste={(e) => {
            const files = pastedFiles([...e.clipboardData.types], [...e.clipboardData.files]);
            if (files.length > 0) {
              e.preventDefault();
              add(files);
            }
          }}
        />
      </div>
    </div>
  );
}

/// The images the owner sent, as thumbnails in a row that wraps, under the owner's words or where a round was given
/// them; a click opens one at full size in a new tab.
export function Thumbs({ ids }: { ids: number[] }) {
  if (ids.length === 0) return null;
  return (
    <div className="thumbs">
      {ids.map((id) => (
        <a key={id} className="thumb" href={imageHref(id)} target="_blank" rel="noreferrer" title={t('image_open')}>
          <img src={imageHref(id)} alt={t('image_alt', { id })} loading="lazy" />
        </a>
      ))}
    </div>
  );
}

/// The words of a form, when it takes words.
export type Words = { text: string; set: (text: string) => void; placeholder: string; rows?: number };

/// The form that answers or says something: what goes above its words (a choice, whom to); its words, which Ctrl or
/// Cmd with Enter sends too; the button that sends, greyed with why while it cannot; and what failed.
export function SendForm({ above, words, label, ready, busy, why, onSend, error }: {
  above?: ReactNode;
  words?: Words;
  label: string;
  ready: boolean;
  busy: boolean;
  /// Why it cannot be sent yet, said on hovering the button.
  why?: string;
  onSend: () => void;
  error: string | null;
}) {
  const can = ready && !busy;
  return (
    <div className="send-form">
      {above}
      {words !== undefined ? (
        <Textarea
          block
          resize="vertical"
          value={words.text}
          rows={words.rows ?? 3}
          placeholder={words.placeholder}
          title={t('send_keys_hint')}
          onChange={(e) => words.set(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && can) {
              e.preventDefault();
              onSend();
            }
          }}
          disabled={busy}
        />
      ) : null}
      <div className="send-form-row">
        <Button variant="primary" onClick={onSend} disabled={!can} title={ready ? undefined : why}>
          {label}
        </Button>
      </div>
      {error !== null ? <Flash variant="danger">{t('request_failed', { error })}</Flash> : null}
    </div>
  );
}
