/**
 * A title for an agent tip from its prompt, so the user only has to write
 * the prompt. Mirrors `src-tauri/src/core/prompt.rs`; both run the vectors
 * in `prompt-vectors.json`. See the Rust module for the full rule:
 * U+FEFF removed; runner blocks and HTML comments (multi-line, or to the end
 * when unclosed) dropped; the first line with text, rules skipped, without
 * Markdown markers (ASCII-digit list markers only); whitespace from the one
 * shared class collapsed; cut to 60 characters at a word boundary, never
 * inside a character cluster, never down to a bare `…`.
 */

export const MAX_DERIVED_TITLE_CHARS = 60;
/** A cut at a space earlier than this would lose too much; cut hard instead. */
const MIN_WORD_CUT_CHARS = 20;
const ELLIPSIS = '…';
const ZWJ = '‍';
/** The whitespace class shared with `prompt.rs` (spelled the same there). */
const WS = '[\\t\\n\\x0B\\x0C\\r \\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]';
const TRAILING_PUNCT = ',;:.-–—';

const re = (pattern: string, flags = 'u'): RegExp => new RegExp(pattern.replace(/WS/g, WS), flags);

const AGENT_BLOCK = re('<!--WS*agent:begin[\\s\\S]*?<!--WS*agent:end(?:WS)*-->|<!--WS*agent:begin[\\s\\S]*', 'gu');
const COMMENT = re('<!--[\\s\\S]*?-->|<!--[\\s\\S]*', 'gu');
const LEAD = re('^(?:#{1,6}WS+|>WS?|[-*+]WS+|[0-9]+[.)]WS+|\\[[ xX]\\]WS+)');
const IMAGE = re('!\\[([^\\]]*)\\]\\([^)]*\\)', 'gu');
const LINK = re('\\[([^\\]]+)\\]\\([^)]*\\)', 'gu');
const EMPHASIS = re('\\*\\*|__|~~|`', 'gu');
const RULE = re('^(?:[-*_=]|WS)+$');
const SPACES = re('WS+', 'gu');
const ONE_SPACE = re(`^${WS}$`);
const EXTEND = re('^[\\p{M}\\u200D\\uFE00-\\uFE0F\\u{1F3FB}-\\u{1F3FF}\\u{E0020}-\\u{E007F}]$');

const isWs = (c: string): boolean => ONE_SPACE.test(c);
const isExtend = (c: string | undefined): boolean => c !== undefined && EXTEND.test(c);

/** Whitespace (class `WS`) collapsed to single spaces and trimmed. */
function collapse(text: string): string {
  return text.replace(SPACES, ' ').replace(/^ +| +$/g, '');
}

function plainLine(line: string): string {
  let text = line;
  for (let stripped = text.replace(LEAD, ''); stripped !== text; stripped = text.replace(LEAD, '')) text = stripped;
  return collapse(text.replace(IMAGE, '$1').replace(LINK, '$1').replace(EMPHASIS, ''));
}

/** `kept` without a cluster the cut split (see `drop_split_cluster` in prompt.rs). */
function dropSplitCluster(kept: string[], next: string | undefined): string[] {
  const out = [...kept];
  if (!isExtend(next) && out[out.length - 1] !== ZWJ) return out;
  for (;;) {
    while (isExtend(out[out.length - 1])) out.pop();
    out.pop();
    if (out[out.length - 1] !== ZWJ) return out;
  }
}

function trimEnd(chars: string[], drop: (c: string) => boolean): string {
  let end = chars.length;
  while (end > 0 && drop(chars[end - 1] ?? '')) end -= 1;
  return chars.slice(0, end).join('');
}

function shorten(text: string): string {
  const chars = Array.from(text);
  if (chars.length <= MAX_DERIVED_TITLE_CHARS) return text;
  const head = chars.slice(0, MAX_DERIVED_TITLE_CHARS);
  let space = -1;
  head.forEach((c, i) => {
    if (isWs(c)) space = i;
  });
  const cut = space >= MIN_WORD_CUT_CHARS ? space : MAX_DERIVED_TITLE_CHARS;
  const kept = trimEnd(dropSplitCluster(head.slice(0, cut), chars[cut]), (c) => isWs(c) || TRAILING_PUNCT.includes(c));
  if (!kept) return `${trimEnd(head, isWs)}${ELLIPSIS}`;
  return `${kept}${ELLIPSIS}`;
}

/** The title for a tip whose prompt is `prompt`; `undefined` when it has no text. */
export function titleFromPrompt(prompt: string): string | undefined {
  const text = prompt.replace(/﻿/g, '').replace(AGENT_BLOCK, '').replace(COMMENT, '');
  for (const raw of text.split(/\r?\n/)) {
    const line = collapse(raw);
    if (!line || RULE.test(line)) continue;
    const plain = plainLine(line);
    if (plain) return shorten(plain);
  }
  return undefined;
}
