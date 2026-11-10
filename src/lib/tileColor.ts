import type { Kind, Tip, Urgency } from '../types';
import { parseNaive } from './time';
import { displayKind } from './question';
import { urgencyOf } from './urgency';

export interface TileColor {
  bg: string;
  fg: string;
  overdue: boolean;
}

/**
 * NERV palette, see docs/DESIGN.md. Every colour is desaturated enough to sit
 * on the same monitor for a whole day; the only saturated thing on the board
 * is the orange accent edge on the next-up tile.
 */
export const KIND_COLORS: Record<Exclude<Kind, 'deadline'>, string> = {
  task: '#4A3B6B', // EVA-01 purple
  event: '#2C5F58', // deep teal
  note: '#3A4356', // slate
  reading: '#8C4A22', // burnt orange
  agent: '#34557A', // steel blue
};

export const DEADLINE_COLORS: Record<Exclude<Urgency, 'overdue'>, string> = {
  none: '#3F6B3A', // EVA-01 green
  later: '#3F6B3A',
  soon: '#A87B1F', // amber
  critical: '#9E2F2A', // EVA-02 red
};

export const OVERDUE_COLOR = '#6E1B2B';
export const DONE_COLOR = '#26272B';
export const LIGHT_FG = '#F2EFE9';
export const DARK_FG = '#1A1A1A';

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

/** WCAG 2 contrast ratio between two hex colours, 1..21. */
export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * Text colour for a tile background: whichever of the light and dark text
 * colours has the higher WCAG contrast against it (light wins a tie). A
 * non-hex value gets light text.
 */
export function foregroundFor(bg: string): string {
  if (!isHexColor(bg)) return LIGHT_FG;
  return contrastRatio(bg, DARK_FG) > contrastRatio(bg, LIGHT_FG) ? DARK_FG : LIGHT_FG;
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
  const kind = displayKind(tip);
  const bg = kind === 'deadline' ? DEADLINE_COLORS[urgency] : KIND_COLORS[kind];
  return { bg, fg: foregroundFor(bg), overdue: false };
}
