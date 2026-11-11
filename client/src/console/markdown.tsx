// A passage of markdown as the item window shows it: paragraphs, blocks of code between fences (a diagram among them,
// shown as its source: the page has no library that draws one), and in a line `code` and **strong** words. The page on
// `main` drew them with its markdown library; the new page has none, and the window's passages need no more.

import { type ReactNode } from 'react';

/// The words of one line or paragraph, with its `code` and **strong** parts.
function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /`([^`]+)`|\*\*([^*]+)\*\*/g;
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > at) out.push(text.slice(at, m.index));
    out.push(m[1] !== undefined ? <code key={m.index}>{m[1]}</code> : <strong key={m.index}>{m[2]}</strong>);
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}

export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  const lines = text.split('\n');
  let para: string[] = [];
  const flush = () => {
    if (para.length > 0) blocks.push(<p key={blocks.length}>{inline(para.join(' '))}</p>);
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith('```')) {
      flush();
      const code: string[] = [];
      for (i += 1; i < lines.length && !lines[i].startsWith('```'); i++) code.push(lines[i]);
      blocks.push(
        <pre key={blocks.length} className="content">
          <code>{code.join('\n')}</code>
        </pre>,
      );
    } else if (line.trim() === '') flush();
    else para.push(line);
  }
  flush();
  return <div className="cw-md">{blocks}</div>;
}
