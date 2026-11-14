// The form of a return as a tree, read from its JSON: each property under its name with its type, a required one
// marked; where a return holds one of several, each in a box with an `or` line between them; a node that holds others
// folds away at a press. A name is drawn by what it is: a kind of return, a tool, the blocks a cell writes under their
// own names, or a field.

import { Fragment, type ReactNode } from 'react';

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

/// What the tree needs to know beside the form: which names are the engine's tools.
interface Known {
  tools: string[];
}

export function FormTree({ text, tools }: { text: string; tools: string[] }) {
  let schema: Schema;
  try {
    schema = JSON.parse(text) as Schema;
  } catch {
    return <pre className="content">{text}</pre>;
  }
  return <div className="form-tree">{inside(schema, null, { tools })}</div>;
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
/// of its properties.
function inside(node: Schema, parent: string | null, known: Known): ReactNode {
  const s = held(node);
  if (s.anyOf) {
    // An alternative that holds nothing, as a plain string, is shown by its type.
    return oneOf(
      s.anyOf.map((alt) =>
        holdsAny(alt) ? inside(alt, parent, known) : <span className="tree-type">{typeOf(alt)}</span>,
      ),
    );
  }
  const properties = Object.entries(s.properties ?? {});
  const required = s.required ?? [];
  const named = (name: string, child: Schema, style: string) => (
    <Item key={name} name={name} schema={child} required={required.includes(name)} style={style} known={known} />
  );
  const style = (name: string) => {
    if (parent === null) return 'tree-kind';
    if (known.tools.includes(name)) return 'tree-tool';
    return '';
  };
  if (s.minProperties === 1 && s.maxProperties === 1) {
    return oneOf(properties.map(([name, child]) => named(name, child, style(name))));
  }
  return (
    <>
      {properties.map(([name, child]) => named(name, child, style(name)))}
      {typeof s.additionalProperties === 'object' && named('additionalProperties', s.additionalProperties, 'tree-own')}
    </>
  );
}

function Item({
  name,
  schema,
  required,
  style,
  known,
}: {
  name: string;
  schema: Schema;
  required: boolean;
  style: string;
  known: Known;
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
      <summary>{head}</summary>
      <div className="tree-children">{inside(schema, name, known)}</div>
    </details>
  );
}
