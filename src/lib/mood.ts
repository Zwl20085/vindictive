/**
 * What Clawd feels, decided from the board. Pure functions only; the sprite
 * in `ui/clawd.ts` turns a mood into pixels.
 */
import type { Language, Tip } from '../types';
import { t, type StringKey } from './i18n';
import { isSnoozed, urgencyOfNaive } from './urgency';

export const CLAWD_MOODS = ['celebrating', 'sleeping', 'worried', 'relaxed', 'walking'] as const;
export type ClawdMood = (typeof CLAWD_MOODS)[number];

/** Clawd sleeps from this hour... */
export const SLEEP_FROM_HOUR = 23;
/** ...until this one. */
export const WAKE_AT_HOUR = 6;
/**
 * More completions than this in one board update look like a folder reload
 * or a sync from another PC, not something the user just did: no party.
 */
export const MAX_CELEBRATED_AT_ONCE = 3;

export interface MoodCounts {
  /** Open tips on the board (not snoozed). */
  open: number;
  /** Those of them that are past due. */
  overdue: number;
}

export interface MoodInput {
  /** Every tip of the board; `undefined` before the first load. */
  tips: readonly Tip[] | undefined;
  now: Date;
  /** A completion was just celebrated and the party is still running. */
  celebrating: boolean;
}

export function isClawdMood(value: unknown): value is ClawdMood {
  return typeof value === 'string' && (CLAWD_MOODS as readonly string[]).includes(value);
}

/** True during Clawd's night-time nap. */
export function isSleepTime(now: Date): boolean {
  const h = now.getHours();
  return h >= SLEEP_FROM_HOUR || h < WAKE_AT_HOUR;
}

/** Open, un-snoozed tips and how many of them are overdue. */
export function moodCounts(tips: readonly Tip[], now: Date): MoodCounts {
  const open = tips.filter((tip) => tip.status === 'open' && !isSnoozed(tip, now));
  const overdue = open.filter((tip) => urgencyOfNaive(tip.due_at, now) === 'overdue');
  return { open: open.length, overdue: overdue.length };
}

/** Priority: celebrating > sleeping > worried > relaxed > walking. */
export function clawdMood({ tips, now, celebrating }: MoodInput): ClawdMood {
  if (celebrating) return 'celebrating';
  if (isSleepTime(now)) return 'sleeping';
  if (!tips) return 'walking';
  const counts = moodCounts(tips, now);
  if (counts.overdue > 0) return 'worried';
  if (counts.open === 0) return 'relaxed';
  return 'walking';
}

/**
 * Ids of tips that were completed between two boards: open before and done
 * now, or a recurring tip that rolled forward (still open, new `done_at`).
 */
export function completedIds(previous: readonly Tip[], next: readonly Tip[]): string[] {
  const before = new Map(previous.map((tip) => [tip.id, tip]));
  return next
    .filter((tip) => {
      const old = before.get(tip.id);
      if (!old || old.status !== 'open') return false;
      if (tip.status === 'done') return true;
      return !!tip.done_at && tip.done_at !== old.done_at;
    })
    .map((tip) => tip.id);
}

/** A few tips were just completed; never on the first board. */
export function shouldCelebrate(previous: readonly Tip[] | undefined, next: readonly Tip[]): boolean {
  if (!previous) return false;
  const count = completedIds(previous, next).length;
  return count > 0 && count <= MAX_CELEBRATED_AT_ONCE;
}

const FEELING_KEY: Record<ClawdMood, StringKey> = {
  celebrating: 'clawdCelebrating',
  sleeping: 'clawdSleeping',
  worried: 'clawdWorried',
  relaxed: 'clawdRelaxed',
  walking: 'clawdWalking',
};

/** Hover text, e.g. `Clawd · 2 overdue` or `Clawd · 全部完成`. */
export function clawdFeeling(mood: ClawdMood, counts: MoodCounts, lang?: Language): string {
  const n = mood === 'worried' ? counts.overdue : counts.open;
  return `Clawd · ${t(FEELING_KEY[mood], lang).replace('{n}', String(n))}`;
}
