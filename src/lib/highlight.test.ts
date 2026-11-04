import { describe, expect, it } from 'vitest';
import { highlightNode, highlightText, wordsFor } from './highlight';

const WORDS = { auto: false };
const AUTO = { words: [], auto: true };

function host(html: string): HTMLElement {
  const div = document.createElement('div');
  div.innerHTML = html;
  return div;
}

/** `[class text]` for every mark under `node`. */
function marks(node: ParentNode): string[] {
  return Array.from(node.querySelectorAll('mark')).map((m) => `${m.className.replace('hl hl-', '')}:${m.textContent}`);
}

function textMarks(text: string, opts: { words: string[]; auto: boolean }): string[] {
  const div = document.createElement('div');
  div.append(highlightText(text, opts));
  return marks(div);
}

describe('keywords', () => {
  it('matches ASCII words case-insensitively on word boundaries', () => {
    const div = host('<p>Review the Thesis draft; thesisx and athesis stay plain.</p>');
    expect(highlightNode(div, { words: ['thesis'], ...WORDS })).toBe(1);
    expect(marks(div)).toEqual(['kw:Thesis']);
    expect(div.querySelector('mark')?.className).toBe('hl hl-kw');
  });

  it('matches CJK entries as substrings', () => {
    expect(textMarks('明天交论文初稿', { words: ['论文'], auto: false })).toEqual(['kw:论文']);
  });

  it('escapes regex metacharacters and matches punctuation as a substring', () => {
    expect(textMarks('Port it to C++ and C.', { words: ['C++', 'a.b'], auto: false })).toEqual(['kw:C++']);
    expect(textMarks('axb', { words: ['a.b'], auto: false })).toEqual([]);
  });

  it('lets the longest entry win on overlap', () => {
    expect(textMarks('deep learning works', { words: ['learning', 'deep learning'], auto: false })).toEqual(['kw:deep learning']);
    expect(textMarks('机器学习模型', { words: ['机器学', '学习模型'], auto: false })).toEqual(['kw:学习模型']);
  });

  it('ignores blank and duplicate entries', () => {
    expect(textMarks('grant grant', { words: ['', '  ', 'Grant', 'grant'], auto: false })).toEqual(['kw:grant', 'kw:grant']);
  });

  it('keeps the text intact', () => {
    const div = document.createElement('div');
    div.append(highlightText('a thesis b', { words: ['thesis'], auto: true }));
    expect(div.textContent).toBe('a thesis b');
  });
});

describe('automatic patterns', () => {
  it('marks dates and times', () => {
    expect(textMarks('On 2026-11-04 at 10:30, or 2026/11/05', AUTO)).toEqual(['date:2026-11-04', 'date:10:30', 'date:2026/11/05']);
    expect(textMarks('by 15 Oct, not Oct 16', AUTO)).toEqual(['date:15 Oct', 'date:Oct 16']);
    expect(textMarks('截止10月15日下午', AUTO)).toEqual(['date:10月15日']);
  });

  it('does not mark numbers that only look like times or dates', () => {
    expect(textMarks('ratio 1:2, version 12345-67-89, 25:99', AUTO)).toEqual([]);
  });

  it('marks tags but not C# or mid-word hashes', () => {
    expect(textMarks('see #thesis and #论文, C# and a#b', AUTO)).toEqual(['tag:#thesis', 'tag:#论文']);
  });

  it('marks bare URLs without trailing punctuation', () => {
    expect(textMarks('Read https://example.org/a?b=1. Then (http://x.io/p).', AUTO)).toEqual(['url:https://example.org/a?b=1', 'url:http://x.io/p']);
  });

  it('marks urgency words only as uppercase whole words', () => {
    expect(textMarks('TODO: fix; todo; TODOS; ASAP! URGENT BLOCKED DUE FIXME', AUTO)).toEqual([
      'urgent:TODO',
      'urgent:ASAP',
      'urgent:URGENT',
      'urgent:BLOCKED',
      'urgent:DUE',
      'urgent:FIXME',
    ]);
    expect(textMarks('紧急：待办被阻塞', AUTO)).toEqual(['urgent:紧急', 'urgent:待办', 'urgent:阻塞']);
  });

  it('is off unless asked for', () => {
    expect(textMarks('TODO 2026-11-04 #tag', { words: [], auto: false })).toEqual([]);
  });

  it('gives keywords precedence over overlapping auto matches', () => {
    expect(textMarks('TODO list due 2026-11-04', { words: ['todo list', '11-04'], auto: true })).toEqual(['kw:TODO list', 'kw:11-04']);
  });
});

describe('highlightNode', () => {
  it('skips code, pre and form fields', () => {
    const div = host('<p>TODO</p><p><code>TODO</code></p><pre>TODO thesis</pre><textarea>TODO</textarea>');
    expect(highlightNode(div, { words: ['thesis'], auto: true })).toBe(1);
    expect(marks(div)).toEqual(['urgent:TODO']);
  });

  it('does not mark URLs inside links but keeps keywords there', () => {
    const div = host('<p><a href="https://a.org">https://a.org thesis</a> and https://b.org</p>');
    highlightNode(div, { words: ['thesis'], auto: true });
    expect(marks(div)).toEqual(['kw:thesis', 'url:https://b.org']);
    expect(div.querySelector('a mark.hl-url')).toBeNull();
  });

  it('does not double-wrap on a second pass', () => {
    const div = host('<p>TODO: thesis by 2026-11-04 #grant</p>');
    const opts = { words: ['thesis'], auto: true };
    expect(highlightNode(div, opts)).toBe(4);
    const html = div.innerHTML;
    expect(highlightNode(div, opts)).toBe(0);
    expect(div.innerHTML).toBe(html);
    expect(div.querySelectorAll('mark mark')).toHaveLength(0);
  });

  it('skips anything inside an existing .hl element', () => {
    const div = host('<span class="hl">thesis</span> thesis');
    expect(highlightNode(div, { words: ['thesis'], auto: false })).toBe(1);
  });

  it('never parses text as HTML', () => {
    const div = document.createElement('div');
    div.textContent = '<img src=x onerror=alert(1)> thesis';
    highlightNode(div, { words: ['thesis'], auto: false });
    expect(div.querySelector('img')).toBeNull();
    expect(div.textContent).toBe('<img src=x onerror=alert(1)> thesis');
  });

  it('returns 0 and changes nothing with no words and auto off', () => {
    const div = host('<p>TODO</p>');
    expect(highlightNode(div, { words: [], auto: false })).toBe(0);
    expect(div.innerHTML).toBe('<p>TODO</p>');
  });
});

describe('wordsFor', () => {
  it('joins the tip list and the global list, dropping blanks', () => {
    expect(wordsFor(['a', ' '], ['b'])).toEqual(['a', 'b']);
    expect(wordsFor(undefined, undefined)).toEqual([]);
  });
});
