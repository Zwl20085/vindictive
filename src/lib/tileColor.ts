import type { Kind, Tip, Urgency } from '../types';
import { parseNaive } from './time';
import { urgencyOf } from './urgency';

export interface TileColor {
  bg: string;
  fg: string;
  overdue: boolean;
}

/** Metro palette from docs/DESIGN.md. */
export const KIND_COLORS: Record<Exclude<Kind, 'deadline'>, string> = {
  task: '#2D89EF',
  event: '#00ABA9',
  note: '#7E3878',
  reading: '#DA532C',
};

export const DEADLINE_COLORS: Record<Exclude<Urgency, 'overdue'>, string> = {
  none: '#1E7145',
  later: '#1E7145',
  soon: '#FFC40D',
  critical: '#EE1111',
};

export const OVERDUE_COLOR = '#B91D47';
export const DONE_COLOR = '#3A3A3D';
export const LIGHT_FG = '#FFFFFF';
export const DARK_FG = '#1A1A1A';

/** Relative luminance threshold above which we use dark text. */
const DARK_TEXT_LUMINANCE = 0.5;

const HEX_RE = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;

export function isHexColor(value: string | undefined): value is string {
  return typeof value === 'string' && HEX_RE.test(value.trim());
}

function expandHex(hex: string): string {
  const raw = hex.trim().slice(1);
  if (raw.length === 6) return raw;
  return raw
    .split('')
    .map((c) => c + c)
    .join('');
}

/** WCAG relative luminance of a hex colour, 0..1. */
export function luminance(hex: string): number {
  const full = expandHex(hex);
  const channel = (i: number): number => {
    const v = parseInt(full.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4);
}

export function foregroundFor(bg: string): string {
  return isHexColor(bg) && luminance(bg) > DARK_TEXT_LUMINANCE ? DARK_FG : LIGHT_FG;
}

/** Decide the tile colour for a tip at `now`. */
export function tileColor(tip: Tip, now: Date): TileColor {
  const urgency = urgencyOf(parseNaive(tip.due_at), now);
  if (tip.status === 'done') return { bg: DONE_COLOR, fg: LIGHT_FG, overdue: false };
  if (isHexColor(tip.color)) {
    const bg = tip.color.trim();
    return { bg, fg: foregroundFor(bg), overdue: urgency === 'overdue' };
  }
  if (urgency === 'overdue') return { bg: OVERDUE_COLOR, fg: LIGHT_FG, overdue: true };
  const bg = tip.kind === 'deadline' ? DEADLINE_COLORS[urgency] : KIND_COLORS[tip.kind];
  return { bg, fg: foregroundFor(bg), overdue: false };
}
