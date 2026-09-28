import type { Urgency } from '../types';
import { DAY_MS, HOUR_MS, parseNaive } from './time';

/** Same thresholds as `src-tauri/src/core/deadline.rs`. */
export const CRITICAL_WINDOW_MS = 48 * HOUR_MS;
export const SOON_WINDOW_MS = 7 * DAY_MS;

export function urgencyOf(due: Date | undefined, now: Date): Urgency {
  if (!due) return 'none';
  const left = due.getTime() - now.getTime();
  if (left <= 0) return 'overdue';
  if (left <= CRITICAL_WINDOW_MS) return 'critical';
  if (left <= SOON_WINDOW_MS) return 'soon';
  return 'later';
}

/** Convenience for the naive string the backend sends. */
export function urgencyOfNaive(dueAt: string | undefined, now: Date): Urgency {
  return urgencyOf(parseNaive(dueAt), now);
}
