// A document the owner reads (a study, a policy, a design) is drawn in one style wherever it shows, in a card of a
// conversation or on a page: each part under a heading in plain words, and every list alike, numbered items, each its
// lead in bold and each other part a labelled paragraph in the text's own style, never a box.

import type { ReactNode } from 'react';

import type { JsonValue } from './api';
import { empty, str } from './labelled';
import { MarkdownInline } from './markdown';

/// One item of a list of a document: its lead, what may mark it, and its other parts, each with its name.
export type DocItem = {
  lead: string;
  parts: Array<[string, JsonValue | undefined]>;
  mark?: ReactNode;
  no?: number;
};

/// A list of a document, as every list of every document the owner reads is drawn: numbered items, each its lead in
/// bold and each other part a labelled paragraph in the text's own style, never a box, since a box says it is what an
/// agent handed back apart from the rest, and the whole document is that.
export function DocList({ items }: { items: DocItem[] }) {
  return (
    <ol className="document-list">
      {items.map((x, i) => (
        <li key={i} value={x.no}>
          <p className="document-lead">
            <strong>
              <MarkdownInline text={x.lead} />
            </strong>
            {x.mark !== undefined ? <> {x.mark}</> : null}
          </p>
          {x.parts
            .filter(([, v]) => !empty(v))
            .map(([name, v]) => (
              <p key={name} className="document-part">
                {name}: <MarkdownInline text={str(v)} />
              </p>
            ))}
        </li>
      ))}
    </ol>
  );
}
