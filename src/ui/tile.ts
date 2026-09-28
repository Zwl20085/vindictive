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
  /** Right-click: show the tile's action menu at the pointer. */
  onMenu?: (id: string, x: number, y: number) => void;
  /** Index for the staggered entry animation; `undefined` means no entry animation. */
  enterIndex?: number;
  /** Resolves a figure path to something an `<img>` can show. */
  resolveImage?: ImageResolver;
}

/** How long the tilt release runs before the board flips. */
export const PRESS_MS = 160;
/** Maximum tilt of a pressed tile, degrees. */
export const TILT_DEG = 9;

/**
 * Metro live-tile tilt: lean the tile toward where it was pressed. Writes
 * `--rx` / `--ry`, which `tile.css` reads in `:active` and in the release
 * animation. Pressing dead centre only scales.
 */
export function tiltFor(button: HTMLElement, clientX: number, clientY: number): { rx: number; ry: number } {
  const rect = button.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return { rx: 0, ry: 0 };
  const dx = ((clientX - rect.left) / rect.width) * 2 - 1; // -1 left … 1 right
  const dy = ((clientY - rect.top) / rect.height) * 2 - 1; // -1 top … 1 bottom
  const clamp = (v: number): number => Math.max(-1, Math.min(1, v));
  return { rx: -clamp(dy) * TILT_DEG, ry: clamp(dx) * TILT_DEG };
}

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
  const { tip, isNextUp, now, columns, onOpen, onMenu, enterIndex, resolveImage } = options;
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
  button.addEventListener('pointerdown', (event) => {
    const { rx, ry } = tiltFor(button, event.clientX, event.clientY);
    button.style.setProperty('--rx', `${rx.toFixed(1)}deg`);
    button.style.setProperty('--ry', `${ry.toFixed(1)}deg`);
  });
  if (onMenu) {
    button.addEventListener('contextmenu', (event) => {
      event.preventDefault();
      event.stopPropagation();
      onMenu(tip.id, event.clientX, event.clientY);
    });
  }
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
