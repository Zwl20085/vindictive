import type { Kind } from '../types';
import { kindLabel } from '../lib/i18n';

/** Monochrome kind glyphs, see docs/DESIGN.md. */
export const KIND_GLYPH: Record<Kind, string> = {
  task: '▢', // ▢
  deadline: '◷', // ◷
  note: '≡', // ≡
  reading: '▤', // ▤
  event: '◈', // ◈
};

/** Localised kind label for the active language. */
export function labelFor(kind: Kind): string {
  return kindLabel(kind);
}
