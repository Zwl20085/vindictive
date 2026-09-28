/**
 * Local wall-clock time helpers. The backend sends naive timestamps
 * (`YYYY-MM-DDTHH:MM:SS`, no zone); we interpret them as local time.
 */
import type { Language } from '../types';
import { formatDateTime, spanUnits, t } from './i18n';

export const MINUTE_MS = 60_000;
export const HOUR_MS = 60 * MINUTE_MS;
export const DAY_MS = 24 * HOUR_MS;
export const WEEK_MS = 7 * DAY_MS;

/** Below this the countdown shows days and hours; at or above, days only. */
const DAYS_ONLY_FROM_MS = 3 * DAY_MS;
const MORNING_HOUR = 9;

const NAIVE_RE = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.\d+)?)?)?$/;

/** Parse a naive local timestamp. Returns `undefined` when malformed. */
export function parseNaive(value: string | undefined | null): Date | undefined {
  if (!value) return undefined;
  const m = NAIVE_RE.exec(value.trim());
  if (!m) return undefined;
  const [, y, mo, d, h = '0', mi = '0', s = '0'] = m;
  const date = new Date(Number(y), Number(mo) - 1, Number(d), Number(h), Number(mi), Number(s));
  return Number.isNaN(date.getTime()) ? undefined : date;
}

/** Format a duration magnitude: `3w`, `5d`, `2d 4h`, `6h`, `45m` (or `3周`, `2天4时`…). */
export function formatSpan(ms: number, lang: Language = 'en'): string {
  const u = spanUnits(lang);
  const abs = Math.abs(ms);
  if (abs >= WEEK_MS) return `${Math.floor(abs / WEEK_MS)}${u.w}`;
  if (abs >= DAYS_ONLY_FROM_MS) return `${Math.floor(abs / DAY_MS)}${u.d}`;
  if (abs >= DAY_MS) {
    const days = Math.floor(abs / DAY_MS);
    const hours = Math.floor((abs - days * DAY_MS) / HOUR_MS);
    return hours > 0 ? `${days}${u.d}${u.sep}${hours}${u.h}` : `${days}${u.d}`;
  }
  if (abs >= HOUR_MS) return `${Math.floor(abs / HOUR_MS)}${u.h}`;
  return `${Math.max(1, Math.floor(abs / MINUTE_MS))}${u.m}`;
}

/** Countdown text for a tile: `2d 4h` or `overdue 2h`. */
export function countdown(due: Date, now: Date, lang: Language = 'en'): string {
  const diff = due.getTime() - now.getTime();
  if (diff <= 0) return `${t('overdue', lang)} ${formatSpan(diff, lang)}`;
  return formatSpan(diff, lang);
}

/** Minutes from `now` until 09:00 tomorrow, for the "Tomorrow" snooze. */
export function minutesUntilTomorrowMorning(now: Date): number {
  const target = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, MORNING_HOUR, 0, 0);
  return Math.max(1, Math.round((target.getTime() - now.getTime()) / MINUTE_MS));
}

/** Human date, e.g. `Oct 15, 23:59`. */
export function formatLocal(date: Date, lang: Language = 'en'): string {
  return formatDateTime(date, lang);
}

/** Time of day only, e.g. `12:03`. */
export function formatClock(date: Date, seconds = false): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  const hm = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  return seconds ? `${hm}:${pad(date.getSeconds())}` : hm;
}

/** Serialise a Date back to the naive backend format. */
export function toNaive(date: Date): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  );
}
