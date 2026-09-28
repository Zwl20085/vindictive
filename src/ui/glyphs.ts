import type { Kind } from '../types';

/** Monochrome kind glyphs, see docs/DESIGN.md. */
export const KIND_GLYPH: Record<Kind, string> = {
  task: '▢', // ▢
  deadline: '◷', // ◷
  note: '≡', // ≡
  reading: '▤', // ▤
  event: '◈', // ◈
};

export const KIND_LABEL: Record<Kind, string> = {
  task: 'Task',
  deadline: 'Deadline',
  note: 'Note',
  reading: 'Reading',
  event: 'Event',
};
