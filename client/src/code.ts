// The kind of each word of code the page draws, so that it is coloured by it: a keyword, a literal or a comment, read
// by Prism in the language its file's name says (decision code-is-coloured-from-prisms-words). Prism only reads the
// words: what it would write as markup is never taken. The languages are those most projects write.
import Prism from 'prismjs';
import 'prismjs/components/prism-markup';
import 'prismjs/components/prism-css';
import 'prismjs/components/prism-clike';
import 'prismjs/components/prism-javascript';
import 'prismjs/components/prism-typescript';
import 'prismjs/components/prism-jsx';
import 'prismjs/components/prism-tsx';
import 'prismjs/components/prism-json';
import 'prismjs/components/prism-rust';
import 'prismjs/components/prism-toml';
import 'prismjs/components/prism-yaml';
import 'prismjs/components/prism-markdown';
import 'prismjs/components/prism-bash';
import 'prismjs/components/prism-python';
import 'prismjs/components/prism-go';
import 'prismjs/components/prism-c';
import 'prismjs/components/prism-cpp';
import 'prismjs/components/prism-java';
import 'prismjs/components/prism-sql';

// Prism colours every block of code on the page by itself once it is loaded, unless told not to; it is told before
// that happens, so that it changes nothing the page draws.
Prism.manual = true;

/// The kinds a word of code is coloured by; a word of none of them is in the text's colour.
export const CODE_KINDS = ['keyword', 'literal', 'comment'] as const;
export type CodeKind = (typeof CODE_KINDS)[number];

/// How Prism reads the words of a language.
export type Grammar = Prism.Grammar;

/// The language of a file, by the end of its name.
const LANGUAGES: Record<string, string> = {
  html: 'markup',
  xml: 'markup',
  svg: 'markup',
  css: 'css',
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  jsx: 'jsx',
  ts: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  tsx: 'tsx',
  json: 'json',
  rs: 'rust',
  toml: 'toml',
  yml: 'yaml',
  yaml: 'yaml',
  md: 'markdown',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  py: 'python',
  go: 'go',
  c: 'c',
  h: 'c',
  cc: 'cpp',
  cpp: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  java: 'java',
  sql: 'sql',
};

/// The kind of word each of Prism's names for a word is, by the name or one of its aliases; `null` for a word whose
/// own words are not of the kind of the word around them, as the code inside a template string.
const KIND_OF: Record<string, CodeKind | null> = {
  keyword: 'keyword',
  atrule: 'keyword',
  important: 'keyword',
  title: 'keyword',
  string: 'literal',
  char: 'literal',
  'template-string': 'literal',
  regex: 'literal',
  number: 'literal',
  boolean: 'literal',
  'attr-value': 'literal',
  url: 'literal',
  code: 'literal',
  comment: 'comment',
  prolog: 'comment',
  doctype: 'comment',
  cdata: 'comment',
  interpolation: null,
};

/// The grammar of the file a path names, where the page knows its language.
export function grammarOf(path: string | null): Grammar | null {
  const end = path === null ? undefined : /\.([^./]+)$/.exec(path)?.[1]?.toLowerCase();
  const language = end === undefined ? undefined : LANGUAGES[end];
  return language === undefined ? null : (Prism.languages[language] ?? null);
}

/// The kind of each character of a text of code, as a number: 0 for none, else one more than its kind's place in
/// `CODE_KINDS`.
export function codeKinds(text: string, grammar: Grammar): Uint8Array {
  const kinds = new Uint8Array(text.length);
  let at = 0;
  const walk = (stream: Prism.TokenStream, outer: number) => {
    if (typeof stream === 'string') {
      kinds.fill(outer, at, at + stream.length);
      at += stream.length;
      return;
    }
    if (Array.isArray(stream)) {
      for (const s of stream) walk(s, outer);
      return;
    }
    const names = [stream.type, ...[stream.alias ?? []].flat()];
    const named = names.find((n) => n in KIND_OF);
    const kind = named === undefined ? undefined : KIND_OF[named];
    walk(stream.content, kind === undefined ? outer : kind === null ? 0 : CODE_KINDS.indexOf(kind) + 1);
  };
  walk(Prism.tokenize(text, grammar), 0);
  return kinds;
}

/// A stretch of a text of code, from `start` to `end`, cut where the kind of its words changes.
export function codeParts(
  text: string,
  kinds: Uint8Array | null,
  start: number,
  end: number,
): { text: string; kind: CodeKind | null }[] {
  if (kinds === null) return [{ text: text.slice(start, end), kind: null }];
  const parts: { text: string; kind: CodeKind | null }[] = [];
  for (let from = start; from < end; ) {
    let to = from + 1;
    while (to < end && kinds[to] === kinds[from]) to += 1;
    parts.push({ text: text.slice(from, to), kind: kinds[from] === 0 ? null : CODE_KINDS[kinds[from] - 1] });
    from = to;
  }
  return parts;
}
