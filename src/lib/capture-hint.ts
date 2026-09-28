/** Syntax hint for the quick-capture bar. Mirrors `core/capture.rs`. */
export const CAPTURE_HINT = '#tag  !high/!low  @today @tomorrow @2026-10-15 09:00  ^"Lab 302"  >note  arXiv/DOI';

export const CAPTURE_PLACEHOLDER = 'What next?';

const DATE_WORDS = new Set(['today', 'tomorrow', 'nextweek', 'tod', 'tom', 'tmr']);
const KINDS = new Set(['task', 'deadline', 'due', 'note', 'reading', 'read', 'paper', 'event', 'meeting']);
const ARXIV_RE = /(?:arxiv\.org\/(?:abs|pdf)\/|arxiv:)?(\d{4}\.\d{4,5})(v\d+)?/i;
const DOI_RE = /10\.\d{4,9}\/\S+/i;
const TIME_RE = /^\d{2}:\d{2}$/;
const DATE_RE = /^\d{4}[-/]\d{2}[-/]\d{2}$/;

interface Parsed {
  title: string[];
  tags: string[];
  priority?: string;
  due?: string;
  location?: string;
  kind?: string;
  paper?: string;
}

function tokenize(input: string): string[] {
  const out: string[] = [];
  let cur = '';
  let quoted = false;
  for (const c of input) {
    if (c === '"') quoted = !quoted;
    else if (/\s/.test(c) && !quoted) {
      if (cur) out.push(cur);
      cur = '';
    } else cur += c;
  }
  if (cur) out.push(cur);
  return out;
}

function parseToken(acc: Parsed, tok: string): Parsed {
  if (tok.startsWith('#') && tok.length > 1) return { ...acc, tags: [...acc.tags, tok.slice(1)] };
  const lower = tok.slice(1).toLowerCase();
  if (tok.startsWith('!') && ['high', 'h', 'low', 'l'].includes(lower)) {
    return { ...acc, priority: lower.startsWith('h') ? 'high' : 'low' };
  }
  if (tok.startsWith('^') && tok.length > 1) return { ...acc, location: tok.slice(1) };
  if (tok.startsWith('>') && KINDS.has(lower)) return { ...acc, kind: lower };
  if (tok.startsWith('@') && (DATE_WORDS.has(lower) || DATE_RE.test(tok.slice(1)))) {
    return { ...acc, due: tok.slice(1) };
  }
  if (acc.due && TIME_RE.test(tok) && !acc.due.includes(':')) return { ...acc, due: `${acc.due} ${tok}` };
  const arxiv = ARXIV_RE.exec(tok);
  if (arxiv && (!/[/:]/.test(tok) || /arxiv/i.test(tok))) return { ...acc, paper: `arXiv ${arxiv[1]}` };
  const doi = DOI_RE.exec(tok);
  if (doi) return { ...acc, paper: `DOI ${doi[0]}` };
  return { ...acc, title: [...acc.title, tok] };
}

/**
 * A one-line preview of how the backend will interpret the capture text,
 * e.g. `Check coil · #lab · high · due tomorrow 09:30 · @ Lab 302`.
 */
export function describeCapture(input: string): string {
  const parsed = tokenize(input).reduce(parseToken, { title: [], tags: [] } as Parsed);
  const parts: string[] = [];
  const title = parsed.title.join(' ').trim();
  if (title) parts.push(title);
  else if (parsed.paper) parts.push(parsed.paper);
  if (parsed.kind) parts.push(parsed.kind);
  if (parsed.tags.length) parts.push(parsed.tags.map((t) => `#${t}`).join(' '));
  if (parsed.priority) parts.push(parsed.priority);
  if (parsed.due) parts.push(`due ${parsed.due}`);
  if (parsed.location) parts.push(`@ ${parsed.location}`);
  if (parsed.paper && title) parts.push(parsed.paper);
  return parts.join(' · ');
}
