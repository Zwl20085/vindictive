import type { Tip } from '../types';
import { countdown, parseNaive } from '../lib/time';
import { spanFor, tileSize } from '../lib/tileSize';
import { tileColor } from '../lib/tileColor';
import { el } from './dom';
import { KIND_GLYPH, KIND_LABEL } from './glyphs';

export interface TileOptions {
  tip: Tip;
  isNextUp: boolean;
  now: Date;
  columns: number;
  onOpen: (id: string) => void;
}

export const NEXT_LABEL = 'NEXT';

function countdownText(tip: Tip, now: Date): string | undefined {
  const due = parseNaive(tip.due_at);
  return due ? countdown(due, now) : undefined;
}

function ariaLabel(tip: Tip, cd: string | undefined, isNextUp: boolean): string {
  const parts = [KIND_LABEL[tip.kind], tip.title];
  if (cd) parts.push(cd);
  if (isNextUp) parts.unshift('Next up');
  return parts.join(', ');
}

function topRow(tip: Tip, cd: string | undefined, isNextUp: boolean): HTMLElement {
  const left = isNextUp ? NEXT_LABEL : cd ?? '';
  return el(
    'span',
    { className: 'tile-top' },
    el('span', { className: isNextUp ? 'tile-next' : 'tile-countdown', text: left }),
    isNextUp && cd ? el('span', { className: 'tile-countdown', text: cd }) : null,
    el('span', { className: 'tile-glyph', 'aria-hidden': 'true', text: KIND_GLYPH[tip.kind] }),
  );
}

function displayTitle(tip: Tip): string {
  if (tip.kind === 'reading' && tip.paper?.title) return tip.paper.title;
  return tip.title;
}

/** Build one tile `<button>`. */
export function renderTile(options: TileOptions): HTMLButtonElement {
  const { tip, isNextUp, now, columns, onOpen } = options;
  const size = tileSize(tip, isNextUp, now);
  const [cols, rows] = spanFor(size, columns);
  const colour = tileColor(tip, now);
  const cd = countdownText(tip, now);

  const button = el(
    'button',
    {
      className: `tile tile-${size}${isNextUp ? ' tile-is-next' : ''}${colour.overdue ? ' tile-overdue' : ''}`,
      type: 'button',
      'data-id': tip.id,
      'aria-label': ariaLabel(tip, cd, isNextUp),
      title: tip.title,
    },
    topRow(tip, cd, isNextUp),
    el('span', { className: 'tile-title', text: displayTitle(tip) }),
  );
  button.style.setProperty('--tile-bg', colour.bg);
  button.style.setProperty('--tile-fg', colour.fg);
  button.style.gridColumn = `span ${cols}`;
  button.style.gridRow = `span ${rows}`;
  button.addEventListener('click', () => onOpen(tip.id));
  return button;
}
