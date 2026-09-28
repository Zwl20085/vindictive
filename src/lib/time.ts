/**
 * Local wall-clock time helpers. The backend sends naive timestamps
 * (`YYYY-MM-DDTHH:MM:SS`, no zone); we interpret them as local time.
 */

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

/** Format a duration magnitude: `3w`, `5d`, `2d 4h`, `6h`, `45m`. */
export function formatSpan(ms: number): string {
  const abs = Math.abs(ms);
  if (abs >= WEEK_MS) return `${Math.floor(abs / WEEK_MS)}w`;
  if (abs >= DAYS_ONLY_FROM_MS) return `${Math.floor(abs / DAY_MS)}d`;
  if (abs >= DAY_MS) {
    const days = Math.floor(abs / DAY_MS);
    const hours = Math.floor((abs - days * DAY_MS) / HOUR_MS);
    return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  }
  if (abs >= HOUR_MS) return `${Math.floor(abs / HOUR_MS)}h`;
  return `${Math.max(1, Math.floor(abs / MINUTE_MS))}m`;
}

/** Countdown text for a tile: `2d 4h` or `overdue 2h`. */
export function countdown(due: Date, now: Date): string {
  const diff = due.getTime() - now.getTime();
  if (diff <= 0) return `overdue ${formatSpan(diff)}`;
  return formatSpan(diff);
}

/** Minutes from `now` until 09:00 tomorrow, for the "Tomorrow" snooze. */
export function minutesUntilTomorrowMorning(now: Date): number {
  const target = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, MORNING_HOUR, 0, 0);
  return Math.max(1, Math.round((target.getTime() - now.getTime()) / MINUTE_MS));
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** Human date, e.g. `Oct 15, 23:59`. */
export function formatLocal(date: Date): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  return `${MONTHS[date.getMonth()]} ${date.getDate()}, ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Time of day only, e.g. `12:03`. */
export function formatClock(date: Date): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Serialise a Date back to the naive backend format. */
export function toNaive(date: Date): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  );
}
