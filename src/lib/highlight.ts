/**
 * Highlight key things in a tip: the user's keywords (a tip's `highlight`
 * list plus the global `highlight_words`) and, optionally, automatic patterns
 * (dates and times, `#tags`, bare URLs, urgency words).
 *
 * Everything is built with `createTextNode` / `createElement`; no string is
 * ever parsed as HTML, so this is safe to run on sanitised markdown.
 */

export interface HighlightOptions {
  /** Keywords and phrases; matched case-insensitively. */
  words: readonly string[];
  /** Also mark dates, times, tags, URLs and urgency words. */
  auto: boolean;
}

/** Kind of a highlight; the mark gets `hl hl-<kind>`. */
export type HighlightKind = 'kw' | 'date' | 'tag' | 'url' | 'urgent';

interface Range {
  start: number;
  end: number;
  kind: HighlightKind;
}

/** A keyword, ready to search with. */
type Matcher = RegExp;

/** Elements whose text is never highlighted. */
const SKIP_TAGS = new Set(['CODE', 'PRE', 'SCRIPT', 'STYLE', 'MARK', 'TEXTAREA', 'INPUT', 'SELECT', 'OPTION', 'NOSCRIPT', 'SVG']);

/** An entry made of ASCII word characters (optionally joined by spaces or hyphens) gets `\b` boundaries. */
const ASCII_WORDY = /^[A-Za-z0-9_](?:[A-Za-z0-9_ -]*[A-Za-z0-9_])?$/;

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Trimmed, non-empty, case-insensitively unique keywords, longest first. */
function normaliseWords(words: readonly string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of words) {
    if (typeof raw !== 'string') continue;
    const word = raw.trim();
    const key = word.toLowerCase();
    if (!word || seen.has(key)) continue;
    seen.add(key);
    out.push(word);
  }
  return out.sort((a, b) => b.length - a.length);
}

function compileWords(words: readonly string[]): Matcher[] {
  return normaliseWords(words).map((word) => {
    const body = escapeRegExp(word);
    return new RegExp(ASCII_WORDY.test(word) ? `\\b${body}\\b` : body, 'gi');
  });
}

/** Every match of `re` in `text` (global regex; zero-length matches skipped). */
function allMatches(re: RegExp, text: string): RegExpExecArray[] {
  const out: RegExpExecArray[] = [];
  re.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m[0].length === 0) {
      re.lastIndex++;
      continue;
    }
    out.push(m);
  }
  return out;
}

function overlaps(a: Range, taken: readonly Range[]): boolean {
  return taken.some((b) => a.start < b.end && b.start < a.end);
}

/** Keyword ranges: the longest entry wins on overlap, then the earliest. */
function keywordRanges(text: string, matchers: readonly Matcher[]): Range[] {
  const candidates: Range[] = [];
  for (const re of matchers) {
    for (const m of allMatches(re, text)) candidates.push({ start: m.index, end: m.index + m[0].length, kind: 'kw' });
  }
  candidates.sort((a, b) => b.end - b.start - (a.end - a.start) || a.start - b.start);
  const taken: Range[] = [];
  for (const c of candidates) if (!overlaps(c, taken)) taken.push(c);
  return taken;
}

const MONTH = '(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|June?|July?|Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)';

/** Automatic patterns, in priority order for ranges that start at the same place. */
const AUTO: ReadonlyArray<{ kind: HighlightKind; re: RegExp; trim?: (match: string) => string }> = [
  {
    kind: 'url',
    re: /\bhttps?:\/\/[^\s<>"'`]+/g,
    // Sentence punctuation after a URL is not part of it; a closing paren
    // only when the URL has no opening one.
    trim: (url) => {
      let out = url.replace(/[.,;:!?'"]+$/, '');
      while (out.endsWith(')') && (out.match(/\(/g)?.length ?? 0) < (out.match(/\)/g)?.length ?? 0)) out = out.slice(0, -1).replace(/[.,;:!?]+$/, '');
      return out;
    },
  },
  { kind: 'date', re: /(?<![\d/-])\d{4}([-/])\d{2}\1\d{2}(?![\d/-])/g },
  { kind: 'date', re: /(?:\d{4}年)?\d{1,2}月\d{1,2}日/g },
  { kind: 'date', re: new RegExp(`\\b\\d{1,2}\\s${MONTH}\\b`, 'g') },
  { kind: 'date', re: new RegExp(`\\b${MONTH}\\s\\d{1,2}\\b(?!:)`, 'g') },
  { kind: 'date', re: /(?<![\d:])(?:[01]?\d|2[0-3]):[0-5]\d(?![\d:])/g },
  { kind: 'tag', re: /(?<![\p{L}\p{N}_#&/])#[\p{L}\p{N}_][\p{L}\p{N}_/-]*/gu },
  { kind: 'urgent', re: /(?<![A-Za-z0-9_])(?:TODO|FIXME|ASAP|URGENT|BLOCKED|DUE)(?![A-Za-z0-9_])/g },
  { kind: 'urgent', re: /紧急|待办|阻塞/g },
];

/** Auto ranges outside `taken` (the keyword ranges): leftmost first, then longest. */
function autoRanges(text: string, taken: readonly Range[], inLink: boolean): Range[] {
  const candidates: Range[] = [];
  for (const { kind, re, trim } of AUTO) {
    if (kind === 'url' && inLink) continue;
    for (const m of allMatches(re, text)) {
      const match = trim ? trim(m[0]) : m[0];
      if (match.length === 0) continue;
      candidates.push({ start: m.index, end: m.index + match.length, kind });
    }
  }
  candidates.sort((a, b) => a.start - b.start || b.end - b.start - (a.end - a.start));
  const out: Range[] = [];
  for (const c of candidates) if (!overlaps(c, taken) && !overlaps(c, out)) out.push(c);
  return out;
}

function rangesFor(text: string, matchers: readonly Matcher[], auto: boolean, inLink: boolean): Range[] {
  const kw = matchers.length ? keywordRanges(text, matchers) : [];
  const all = auto ? [...kw, ...autoRanges(text, kw, inLink)] : kw;
  return all.sort((a, b) => a.start - b.start);
}

function mark(doc: Document, text: string, kind: HighlightKind): HTMLElement {
  const node = doc.createElement('mark');
  node.className = `hl hl-${kind}`;
  node.append(doc.createTextNode(text));
  return node;
}

/** `text` split into plain text nodes and marks for `ranges` (sorted, disjoint). */
function build(doc: Document, text: string, ranges: readonly Range[]): DocumentFragment {
  const fragment = doc.createDocumentFragment();
  let at = 0;
  for (const r of ranges) {
    if (r.start > at) fragment.append(doc.createTextNode(text.slice(at, r.start)));
    fragment.append(mark(doc, text.slice(r.start, r.end), r.kind));
    at = r.end;
  }
  if (at < text.length) fragment.append(doc.createTextNode(text.slice(at)));
  return fragment;
}

/** Why a text node is left alone, or whether it sits inside a link. */
function context(node: Node, root: Node): 'skip' | 'link' | 'plain' {
  let inLink = false;
  for (let p: Node | null = node.parentNode; p; p = p.parentNode) {
    if (p.nodeType === 1) {
      const element = p as Element;
      if (SKIP_TAGS.has(element.tagName.toUpperCase())) return 'skip';
      if (element.classList.contains('hl')) return 'skip';
      if (element.tagName.toUpperCase() === 'A') inLink = true;
    }
    if (p === root) break;
  }
  return inLink ? 'link' : 'plain';
}

/**
 * Wrap every highlight under `root` in `<mark class="hl hl-…">`. Text inside
 * code, preformatted blocks, form fields and existing marks is left alone, so
 * a second pass adds nothing; bare URLs are not marked inside links. Returns
 * the number of marks added.
 */
export function highlightNode(root: Node, opts: HighlightOptions): number {
  const matchers = compileWords(opts.words);
  if (!matchers.length && !opts.auto) return 0;
  const doc = root.ownerDocument ?? (root as Document);
  const texts: Text[] = [];
  if (root.nodeType === 3) {
    texts.push(root as Text);
  } else {
    const walker = doc.createTreeWalker(root, 4 /* NodeFilter.SHOW_TEXT */);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) texts.push(n as Text);
  }
  let count = 0;
  for (const text of texts) {
    const value = text.data;
    if (!value.trim() || !text.parentNode) continue;
    const where = context(text, root);
    if (where === 'skip') continue;
    const ranges = rangesFor(value, matchers, opts.auto, where === 'link');
    if (!ranges.length) continue;
    count += ranges.length;
    text.parentNode.replaceChild(build(doc, value, ranges), text);
  }
  return count;
}

/** `text` as a fragment of text nodes and highlight marks, for plain strings. */
export function highlightText(text: string, opts: HighlightOptions, doc: Document = document): DocumentFragment {
  const matchers = compileWords(opts.words);
  const ranges = matchers.length || opts.auto ? rangesFor(text, matchers, opts.auto, false) : [];
  return build(doc, text, ranges);
}

/** The effective keyword list for a tip: its own `highlight` then the global words. */
export function wordsFor(own: readonly string[] | undefined, global: readonly string[] | undefined): string[] {
  return [...(own ?? []), ...(global ?? [])].filter((w): w is string => typeof w === 'string' && w.trim().length > 0);
}
