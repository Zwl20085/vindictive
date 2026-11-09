import type { Tip } from '../types';
import { AGENT_BADGE, agentStateLabel } from '../lib/agent';
import { locale, t } from '../lib/i18n';
import { isRelativeSrc } from '../lib/markdown';
import { countdown, parseNaive } from '../lib/time';
import { spanFor, tileSize } from '../lib/tileSize';
import { tileColor } from '../lib/tileColor';
import { highlightText, wordsFor } from '../lib/highlight';
import { el } from './dom';
import { KIND_GLYPH, labelFor } from './glyphs';
import type { VersionedResolver } from './images';
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
  /** Resolves a figure path to something an `<img>` can show (cached per tip sha). */
  resolveImage?: VersionedResolver;
  /** Global `highlight_words`; joined with the tip's own `highlight` list. */
  highlightWords?: readonly string[];
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
  if (tip.agent_state) parts.push(agentStateLabel(tip.agent_state));
  if (cd) parts.push(cd);
  if (isNextUp) parts.unshift(t('nextUp'));
  if (warningsOf(tip).length) parts.push(t('tileWarnings'));
  return parts.join(', ');
}

/** Parse warnings the backend attached to the tip (empty strings dropped). */
function warningsOf(tip: Tip): string[] {
  return (tip.warnings ?? []).filter((w) => w.trim().length > 0);
}

/** The agent state badge: an emoji with its meaning in words for hover and screen readers. */
function agentBadge(tip: Tip): HTMLElement | null {
  if (!tip.agent_state) return null;
  const words = agentStateLabel(tip.agent_state);
  return el('span', {
    className: 'tile-agent-badge',
    role: 'img',
    'aria-label': words,
    title: words,
    text: AGENT_BADGE[tip.agent_state],
  });
}

function topRow(tip: Tip, cd: string | undefined, isNextUp: boolean): HTMLElement {
  return el(
    'span',
    { className: 'tile-top' },
    isNextUp ? el('span', { className: 'tile-next', text: t('next') }) : null,
    agentBadge(tip),
    cd ? el('span', { className: 'tile-countdown', text: cd }) : null,
    warningsOf(tip).length ? el('span', { className: 'tile-warn', 'aria-hidden': 'true', text: '!' }) : null,
    el('span', { className: 'tile-glyph', 'aria-hidden': 'true', text: KIND_GLYPH[tip.kind] }),
  );
}

function displayTitle(tip: Tip): string {
  if (tip.kind === 'reading' && tip.paper?.title) return tip.paper.title;
  return tip.title;
}

/** First figure of a tip, shown faintly on md / wide tiles. */
function thumbnail(tip: Tip, resolve: VersionedResolver | undefined): HTMLElement | null {
  const src = tip.images?.[0];
  if (!src || !resolve) return null;
  const img = el('img', { className: 'tile-figure-img', alt: '', 'aria-hidden': 'true', draggable: 'false', decoding: 'async', loading: 'lazy' });
  const frame = el('span', { className: 'tile-figure', 'aria-hidden': 'true' }, img);
  const apply = (url: string): void => {
    img.src = url;
    frame.classList.add('tile-figure-ready');
  };
  if (isRelativeSrc(src)) {
    resolve(src, tip.sha)
      .then(apply)
      .catch(() => frame.remove());
  } else {
    apply(src);
  }
  return frame;
}

/**
 * Everything about a tile except its countdown text. Two renders with the
 * same signature can share one DOM node; only the countdown is refreshed
 * (see `updateTile`). A different signature means the tile is rebuilt.
 */
export function tileSignature(tip: Tip, isNextUp: boolean, now: Date, columns: number, highlightWords: readonly string[] = []): string {
  const size = tileSize(tip, isNextUp, now);
  const [cols, rows] = spanFor(size, columns);
  const colour = tileColor(tip, now);
  return JSON.stringify([
    tip.sha ?? '',
    tip.kind,
    tip.status,
    displayTitle(tip),
    tip.images?.[0] ?? '',
    size,
    cols,
    rows,
    isNextUp,
    colour.overdue,
    colour.bg,
    colour.fg,
    countdownText(tip, now) !== undefined,
    warningsOf(tip),
    wordsFor(tip.highlight, highlightWords),
  ]);
}

/**
 * Refresh a kept tile in place: countdown text and accessible name. Returns
 * `false` (and changes nothing) when the tile's signature differs, in which
 * case the caller must replace it with a fresh `renderTile`.
 */
export function updateTile(
  button: HTMLElement,
  tip: Tip,
  isNextUp: boolean,
  now: Date,
  columns: number,
  highlightWords: readonly string[] = [],
): boolean {
  if (button.dataset.id !== tip.id || button.dataset.sig !== tileSignature(tip, isNextUp, now, columns, highlightWords)) return false;
  const cd = countdownText(tip, now);
  const label = button.querySelector('.tile-countdown');
  if (label && cd !== undefined && label.textContent !== cd) label.textContent = cd;
  const aria = ariaLabel(tip, cd, isNextUp);
  if (button.getAttribute('aria-label') !== aria) button.setAttribute('aria-label', aria);
  return true;
}

/** Build one tile `<button>`. */
export function renderTile(options: TileOptions): HTMLButtonElement {
  const { tip, isNextUp, now, columns, onOpen, onMenu, enterIndex, resolveImage, highlightWords = [] } = options;
  const size = tileSize(tip, isNextUp, now);
  const [cols, rows] = spanFor(size, columns);
  const colour = tileColor(tip, now);
  const cd = countdownText(tip, now);
  const classes = ['tile', `tile-${size}`];
  if (isNextUp) classes.push('tile-is-next');
  if (colour.overdue) classes.push('tile-overdue');
  if (tip.status === 'done') classes.push('tile-done');
  if (tip.agent_state) classes.push(`tile-agent-${tip.agent_state}`);
  if (enterIndex !== undefined) classes.push('tile-enter');

  const button = el(
    'button',
    {
      className: classes.join(' '),
      type: 'button',
      'data-id': tip.id,
      'data-sig': tileSignature(tip, isNextUp, now, columns, highlightWords),
      'data-warn': warningsOf(tip).length ? String(warningsOf(tip).length) : undefined,
      'aria-label': ariaLabel(tip, cd, isNextUp),
      title: tip.title,
      draggable: 'true',
    },
    size === 'sm' ? null : thumbnail(tip, resolveImage),
    topRow(tip, cd, isNextUp),
    // Keywords only: tiles stay quiet, so no automatic patterns here.
    el('span', { className: 'tile-title' }, highlightText(displayTitle(tip), { words: wordsFor(tip.highlight, highlightWords), auto: false })),
  );
  button.style.setProperty('--tile-bg', colour.bg);
  button.style.setProperty('--tile-fg', colour.fg);
  if (enterIndex !== undefined) {
    button.style.setProperty('--i', String(enterIndex));
    // Kept nodes may be moved in the DOM later; drop the entry class once it
    // has played so a move never replays the entrance.
    const entered = (event: AnimationEvent): void => {
      if (event.target !== button || event.animationName !== 'tile-enter') return;
      button.classList.remove('tile-enter');
      button.removeEventListener('animationend', entered);
    };
    button.addEventListener('animationend', entered);
  }
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
