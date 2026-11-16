// A part of the form of a return as a tree, read from its JSON: each property under its name with its type, a required
// one marked; where a return holds one of several, each in a box with an `or` line between them; a node that holds
// others folds away at a press. A name is drawn by what it is: a kind of return, a tool, a block, what a field holds
// under names of its own, or a field.

import { Fragment, type ReactNode } from 'react';

import { FoldMark } from './parts';

/// The part of JSON Schema the form is written in.
interface Schema {
  type?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  items?: Schema;
  anyOf?: Schema[];
  additionalProperties?: boolean | Schema;
  minProperties?: number;
  maxProperties?: number;
  enum?: string[];
}

/// What the names at the top of the tree are, by the key of the part of the form it draws: of `tools`, the flow's
/// tools; of `outputs`, the blocks of a line; of `form`, the kinds of return.
function topStyle(part: string): string {
  if (part === 'tools') return 'tree-tool';
  if (part === 'outputs') return 'tree-own';
  return 'tree-kind';
}

/// The part of the form keyed so in the files, as a tree.
export function FormTree({ text, part }: { text: string; part: string }) {
  let schema: Schema;
  try {
    schema = JSON.parse(text) as Schema;
  } catch {
    return <pre className="content">{text}</pre>;
  }
  return <div className="form-tree">{inside(schema, null, topStyle(part))}</div>;
}

/// The type of a node as the form writes it: an array by what it holds.
function typeOf(s: Schema): string {
  if (s.type === 'array' && s.items) {
    const items = s.items;
    if (items.anyOf || items.type === 'object') return 'array of object';
    return `${items.type ?? ''}[]`;
  }
  const values = s.enum ? ` [${s.enum.join(', ')}]` : '';
  return `${s.type ?? ''}${values}`;
}

/// What a node holds: of an array, what each item holds.
function held(s: Schema): Schema {
  return s.type === 'array' && s.items ? s.items : s;
}

function holdsAny(s: Schema): boolean {
  const h = held(s);
  return Boolean(
    h.anyOf || (h.properties && Object.keys(h.properties).length > 0) || typeof h.additionalProperties === 'object',
  );
}

/// Each of several, one of which a return holds, in a box, with an `or` line between two.
function oneOf(boxes: ReactNode[]) {
  return boxes.map((box, i) => (
    <Fragment key={i}>
      {i > 0 && <div className="tree-or">or</div>}
      <div className="tree-box">{box}</div>
    </Fragment>
  ));
}

/// What a node holds, laid out: one of its alternatives, or one of its properties where it may hold one only, or each
/// of its properties; the names at the top of the tree in the style of what they are.
function inside(node: Schema, parent: string | null, top: string): ReactNode {
  const s = held(node);
  if (s.anyOf) {
    // An alternative that holds nothing, as a plain string, is shown by its type.
    return oneOf(
      s.anyOf.map((alt) =>
        holdsAny(alt) ? inside(alt, parent, top) : <span className="tree-type">{typeOf(alt)}</span>,
      ),
    );
  }
  const properties = Object.entries(s.properties ?? {});
  const required = s.required ?? [];
  const named = (name: string, child: Schema, style: string) => (
    <Item key={name} name={name} schema={child} required={required.includes(name)} style={style} top={top} />
  );
  const style = parent === null ? top : '';
  if (s.minProperties === 1 && s.maxProperties === 1) {
    return oneOf(properties.map(([name, child]) => named(name, child, style)));
  }
  return (
    <>
      {properties.map(([name, child]) => named(name, child, style))}
      {typeof s.additionalProperties === 'object' && named('additionalProperties', s.additionalProperties, 'tree-own')}
    </>
  );
}

function Item({
  name,
  schema,
  required,
  style,
  top,
}: {
  name: string;
  schema: Schema;
  required: boolean;
  style: string;
  top: string;
}) {
  const head = (
    <>
      <span className={style}>{name}</span>
      <span className="tree-type">{typeOf(schema)}</span>
      {required && <span className="tree-required">*</span>}
    </>
  );
  if (!holdsAny(schema)) return <div className="tree-leaf">{head}</div>;
  return (
    <details open>
      <summary>
        <FoldMark />
        {head}
      </summary>
      <div className="tree-children">{inside(schema, name, top)}</div>
    </details>
  );
}
