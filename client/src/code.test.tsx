// The kind of each word of code the page colours, as Prism reads it in the language a file's name or a fence names
// (decision code-is-coloured-from-prisms-words), and the words drawn as the page's own parts, never as markup.

import Prism from 'prismjs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { CodeBlock, codeKinds, codeParts, grammarNamed, grammarOf } from './code';

/// A text of code as its stretches of one kind, each `kind:words`, or the words alone where they are of none.
const kinds = (text: string, language: string, start = 0, end = text.length) => {
  const grammar = grammarNamed(language);
  if (grammar === null) throw new Error(`no grammar ${language}`);
  return codeParts(text, codeKinds(text, grammar), start, end).map((p) => (p.kind === null ? p.text : `${p.kind}:${p.text}`));
};

describe('the language of code', () => {
  it("is found by the end of a file's name, in any case, and none for a name with no end the page knows", () => {
    expect(grammarOf('crates/web/src/lib.rs')).toBe(Prism.languages.rust);
    expect(grammarOf('client/src/App.TSX')).toBe(Prism.languages.tsx);
    expect(grammarOf('flow/project.toml')).toBe(Prism.languages.toml);
    expect(grammarOf('Makefile')).toBeNull();
    expect(grammarOf('notes.unknown')).toBeNull();
    expect(grammarOf('a.dir/README')).toBeNull();
    expect(grammarOf(null)).toBeNull();
  });

  it("is found by a fence's name, the language's own or a file's end, and none for one the page does not know", () => {
    expect(grammarNamed('rust')).toBe(Prism.languages.rust);
    expect(grammarNamed('rs')).toBe(Prism.languages.rust);
    expect(grammarNamed('Python')).toBe(Prism.languages.python);
    expect(grammarNamed('sh')).toBe(Prism.languages.bash);
    expect(grammarNamed('cobol')).toBeNull();
    expect(grammarNamed('')).toBeNull();
    // Names of Prism's own that are no language are none.
    expect(grammarNamed('extend')).toBeNull();
    expect(grammarNamed('toString')).toBeNull();
  });
});

describe('the kinds of the words of code', () => {
  it('are keywords, literals and comments, and every other word is of none', () => {
    expect(kinds('let ms = 1000; // a second', 'rust')).toEqual([
      'keyword:let',
      ' ms = ',
      'literal:1000',
      '; ',
      'comment:// a second',
    ]);
    expect(kinds('name = "wf-agents"', 'toml')).toEqual(['name = ', 'literal:"wf-agents"']);
  });

  it('are of none inside the code a template string holds, the string itself a literal', () => {
    expect(kinds('const a = `x ${b} y`;', 'typescript')).toEqual([
      'keyword:const',
      ' a = ',
      'literal:`x ',
      '${b}',
      'literal: y`',
      ';',
    ]);
  });

  it('of a stretch are read from the whole text, so that a comment over several lines is one in each of them', () => {
    const text = '/* one\ntwo */ let y = 2;';
    const second = text.indexOf('two');
    expect(kinds(text, 'typescript', second, text.length)).toEqual(['comment:two */', ' ', 'keyword:let', ' y = ', 'literal:2', ';']);
    // Read alone, the line would not be a comment.
    expect(kinds('two */ let y = 2;', 'typescript')[0]).not.toBe('comment:two */');
  });

  it('are drawn as spans of the page, what the code says drawn as text and never as markup', () => {
    const drawn = renderToStaticMarkup(<CodeBlock text={'let s = "<b>x</b>";'} grammar={grammarNamed('rust')} />);
    expect(drawn).toBe(
      '<pre class="content"><code><span class="code-keyword">let</span> s = <span class="code-literal">&quot;&lt;b&gt;x&lt;/b&gt;&quot;</span>;</code></pre>',
    );
    expect(renderToStaticMarkup(<CodeBlock text="let s = 1;" grammar={null} />)).toBe(
      '<pre class="content"><code>let s = 1;</code></pre>',
    );
  });
});
