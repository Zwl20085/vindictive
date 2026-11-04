/** Syntax hint and parser for the quick-capture bar. Mirrors `core/capture.rs`. */
import type { Kind, TipEdit } from '../types';

export const CAPTURE_HINT = '#tag  !high/!low  @today @tomorrow @2026-10-15 09:00  ^"Lab 302"  >note  arXiv/DOI';

export const CAPTURE_PLACEHOLDER = 'What next?';

const DATE_WORDS = new Set(['today', 'tomorrow', 'nextweek', 'tod', 'tom', 'tmr']);
const KINDS = new Set(['task', 'deadline', 'due', 'note', 'reading', 'read', 'paper', 'event', 'meeting']);
const ARXIV_RE = /(?:arxiv\.org\/(?:abs|pdf)\/|arxiv:)?(\d{4}\.\d{4,5})(v\d+)?/i;
const DOI_RE = /10\.\d{4,9}\/\S+/i;
const TIME_RE = /^\d{1,2}:\d{2}$/;
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
    if (c === '"' || c === '“' || c === '”') quoted = !quoted;
    else if (/\s/.test(c) && !quoted) {
      if (cur) out.push(cur);
      cur = '';
    } else cur += c;
  }
  if (cur) out.push(cur);
  return out;
}

/** Full-width forms typed under a Chinese IME are treated as their ASCII marker. */
function normaliseMarker(tok: string): string {
  const map: Record<string, string> = { '＃': '#', '！': '!', '＠': '@', '＾': '^', '＞': '>' };
  const first = tok.charAt(0);
  return map[first] ? map[first] + tok.slice(1) : tok;
}

function parseToken(acc: Parsed, raw: string): Parsed {
  const tok = normaliseMarker(raw);
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

/** `>kind` words and the kind each one means. */
const KIND_WORDS: Readonly<Record<string, Kind>> = {
  task: 'task',
  deadline: 'deadline',
  due: 'deadline',
  note: 'note',
  reading: 'reading',
  read: 'reading',
  paper: 'reading',
  event: 'event',
  meeting: 'event',
};

const pad2 = (n: number): string => String(n).padStart(2, '0');

/** `today` / `tomorrow` / `nextweek` (and short forms) or `YYYY-MM-DD` / `YYYY/MM/DD` as `YYYY-MM-DD`; `undefined` if not a real date. */
export function resolveCaptureDate(word: string, now: Date = new Date()): string | undefined {
  const w = word.toLowerCase();
  const offset = w === 'today' || w === 'tod' ? 0 : w === 'tomorrow' || w === 'tom' || w === 'tmr' ? 1 : w === 'nextweek' ? 7 : undefined;
  if (offset !== undefined) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() + offset);
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  }
  const m = /^(\d{4})[-/](\d{2})[-/](\d{2})$/.exec(word);
  if (!m) return undefined;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(y, mo - 1, d);
  if (date.getFullYear() !== y || date.getMonth() !== mo - 1 || date.getDate() !== d) return undefined;
  return `${m[1]}-${m[2]}-${m[3]}`;
}

/** `9:30` → `09:30`; `undefined` for anything that is not a clock time. */
function clockTime(word: string): string | undefined {
  if (!TIME_RE.test(word)) return undefined;
  const [h, m] = word.split(':').map(Number);
  if (h === undefined || m === undefined || h > 23 || m > 59) return undefined;
  return `${pad2(h)}:${pad2(m)}`;
}

/**
 * The capture line as form fields, with the same token rules as
 * `describeCapture`: `#tag`, `!high`/`!low`, `@today`/`@tomorrow`/`@nextweek`/
 * `@YYYY-MM-DD [HH:MM]`, `^place` (quotes group words), `>kind`. Date words
 * resolve against `now`. arXiv ids and DOIs stay in the title, where the
 * backend finds them; every other word is the title too. Only the fields the
 * line sets are present.
 */
export function parseCapture(text: string, now: Date = new Date()): Partial<TipEdit> & { title: string } {
  const title: string[] = [];
  const tags: string[] = [];
  const out: Partial<TipEdit> = {};
  let dueDate: string | undefined;
  let dueTime: string | undefined;
  for (const raw of tokenize(text)) {
    const tok = normaliseMarker(raw);
    const rest = tok.slice(1);
    const lower = rest.toLowerCase();
    const marker = tok.charAt(0);
    if (marker === '#' && rest) {
      if (!tags.includes(rest)) tags.push(rest);
      continue;
    }
    if (marker === '!' && ['high', 'h', 'low', 'l'].includes(lower)) {
      out.priority = lower.startsWith('h') ? 'high' : 'low';
      continue;
    }
    if (marker === '^' && rest) {
      out.location = rest;
      continue;
    }
    const kind = marker === '>' ? KIND_WORDS[lower] : undefined;
    if (kind) {
      out.kind = kind;
      continue;
    }
    const date = marker === '@' ? resolveCaptureDate(rest, now) : undefined;
    if (date) {
      dueDate = date;
      dueTime = undefined;
      continue;
    }
    const time = dueDate && !dueTime ? clockTime(tok) : undefined;
    if (time) {
      dueTime = time;
      continue;
    }
    title.push(raw);
  }
  if (dueDate) out.due = dueTime ? `${dueDate} ${dueTime}` : dueDate;
  if (tags.length) out.tags = tags;
  return { ...out, title: title.join(' ').trim() };
}
