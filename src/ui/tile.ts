import type { Tip } from '../types';
import { locale, t } from '../lib/i18n';
import { isRelativeSrc, type ImageResolver } from '../lib/markdown';
import { countdown, parseNaive } from '../lib/time';
import { spanFor, tileSize } from '../lib/tileSize';
import { tileColor } from '../lib/tileColor';
import { el } from './dom';
import { KIND_GLYPH, labelFor } from './glyphs';
import { animate } from './motion';

export interface TileOptions {
  tip: Tip;
  isNextUp: boolean;
  now: Date;
  columns: number;
  onOpen: (id: string) => void;
  /** Index for the staggered entry animation; `undefined` means no entry animation. */
  enterIndex?: number;
  /** Resolves a figure path to something an `<img>` can show. */
  resolveImage?: ImageResolver;
}

/** How long the press flash runs before the board flips. */
export const PRESS_MS = 160;

function countdownText(tip: Tip, now: Date): string | undefined {
  const due = parseNaive(tip.due_at);
  return due ? countdown(due, now, locale()) : undefined;
}

function ariaLabel(tip: Tip, cd: string | undefined, isNextUp: boolean): string {
  const parts = [labelFor(tip.kind), tip.title];
  if (cd) parts.push(cd);
  if (isNextUp) parts.unshift(t('nextUp'));
  return parts.join(', ');
}

function topRow(tip: Tip, cd: string | undefined, isNextUp: boolean): HTMLElement {
  return el(
    'span',
    { className: 'tile-top' },
    isNextUp ? el('span', { className: 'tile-next', text: t('next') }) : null,
    cd ? el('span', { className: 'tile-countdown', text: cd }) : null,
    el('span', { className: 'tile-glyph', 'aria-hidden': 'true', text: KIND_GLYPH[tip.kind] }),
  );
}

function displayTitle(tip: Tip): string {
  if (tip.kind === 'reading' && tip.paper?.title) return tip.paper.title;
  return tip.title;
}

/** First figure of a tip, shown faintly on md / wide tiles. */
function thumbnail(tip: Tip, resolve: ImageResolver | undefined): HTMLElement | null {
  const src = tip.images?.[0];
  if (!src || !resolve) return null;
  const img = el('img', { className: 'tile-figure-img', alt: '', 'aria-hidden': 'true', draggable: 'false' });
  const frame = el('span', { className: 'tile-figure', 'aria-hidden': 'true' }, img);
  const apply = (url: string): void => {
    img.src = url;
    frame.classList.add('tile-figure-ready');
  };
  if (isRelativeSrc(src)) {
    resolve(src)
      .then(apply)
      .catch(() => frame.remove());
  } else {
    apply(src);
  }
  return frame;
}

/** Build one tile `<button>`. */
export function renderTile(options: TileOptions): HTMLButtonElement {
  const { tip, isNextUp, now, columns, onOpen, enterIndex, resolveImage } = options;
  const size = tileSize(tip, isNextUp, now);
  const [cols, rows] = spanFor(size, columns);
  const colour = tileColor(tip, now);
  const cd = countdownText(tip, now);
  const classes = ['tile', `tile-${size}`];
  if (isNextUp) classes.push('tile-is-next');
  if (colour.overdue) classes.push('tile-overdue');
  if (tip.status === 'done') classes.push('tile-done');
  if (enterIndex !== undefined) classes.push('tile-enter');

  const button = el(
    'button',
    {
      className: classes.join(' '),
      type: 'button',
      'data-id': tip.id,
      'aria-label': ariaLabel(tip, cd, isNextUp),
      title: tip.title,
    },
    size === 'sm' ? null : thumbnail(tip, resolveImage),
    topRow(tip, cd, isNextUp),
    el('span', { className: 'tile-title', text: displayTitle(tip) }),
  );
  button.style.setProperty('--tile-bg', colour.bg);
  button.style.setProperty('--tile-fg', colour.fg);
  if (enterIndex !== undefined) button.style.setProperty('--i', String(enterIndex));
  button.style.gridColumn = `span ${cols}`;
  button.style.gridRow = `span ${rows}`;
  let opening = false;
  button.addEventListener('click', () => {
    if (opening) return;
    opening = true;
    void animate(button, 'tile-pressed', PRESS_MS).then(() => {
      opening = false;
      onOpen(tip.id);
    });
  });
  return button;
}
