import type { Settings, Tip } from '../types';
import { locale, t } from '../lib/i18n';
import { countdown, formatLocal, minutesUntilTomorrowMorning, parseNaive } from '../lib/time';
import { DEADLINE_COLORS, KIND_COLORS, OVERDUE_COLOR, tileColor } from '../lib/tileColor';
import { TILE_SIZES, tileSize, type TileSize } from '../lib/tileSize';
import { isRelativeSrc, renderMarkdown, resolveImages, type ImageResolver } from '../lib/markdown';
import { el } from './dom';
import { KIND_GLYPH, labelFor } from './glyphs';
import { showLightbox } from './lightbox';
import { animate } from './motion';

export const SNOOZE_HOUR_MINUTES = 60;
/** Length of the strike-through played before a tip is marked done. */
export const DONE_MS = 380;

/** A second click within this window confirms a delete. */
export const CONFIRM_MS = 4000;

export interface DetailActions {
  onDone: (id: string) => void;
  onReopen: (id: string) => void;
  onSnooze: (id: string, minutes: number) => void;
  onDelete: (id: string) => void;
  onEditLocal: (id: string) => void;
  onSetColor: (id: string, color: string | null) => void;
  onSetSize: (id: string, size: TileSize | null) => void;
  onOpenLink: (url: string) => void;
  /** Show the tip's file in Explorer. */
  onReveal: (id: string) => void;
  onBack: () => void;
  resolveImage: ImageResolver;
}

export interface DetailOptions {
  tip: Tip;
  settings: Settings;
  now: Date;
  actions: DetailActions;
  isNextUp?: boolean;
}

function header(tip: Tip, now: Date): HTMLElement {
  const due = parseNaive(tip.due_at);
  const sub = due ? `${countdown(due, now, locale())} · ${formatLocal(due, locale())}` : labelFor(tip.kind);
  return el(
    'header',
    { className: 'detail-header' },
    el('span', { className: 'detail-kind' }, el('span', { 'aria-hidden': 'true', text: KIND_GLYPH[tip.kind] }), ` ${labelFor(tip.kind)}`),
    el('h1', { className: 'detail-title' }, el('span', { className: 'detail-title-text', text: tip.title })),
    el('p', { className: 'detail-sub', text: sub }),
  );
}

function metaRow(key: string, value: string): HTMLElement {
  return el('li', { className: 'detail-meta-row' }, el('span', { className: 'detail-meta-key', text: key }), value);
}

function metaRows(tip: Tip): HTMLElement | null {
  const rows: HTMLElement[] = [];
  if (tip.location) rows.push(metaRow(t('where'), tip.location));
  if (tip.repeat) rows.push(metaRow(t('repeat'), tip.repeat));
  if (tip.tags?.length) rows.push(metaRow(t('tags'), tip.tags.map((x) => `#${x}`).join(' ')));
  if (tip.snoozed_until_at) {
    const until = parseNaive(tip.snoozed_until_at);
    if (until) rows.push(metaRow(t('snoozed'), formatLocal(until, locale())));
  }
  return rows.length ? el('ul', { className: 'detail-meta' }, ...rows) : null;
}

function paperBlock(tip: Tip, onOpenLink: (url: string) => void): HTMLElement | null {
  const paper = tip.paper;
  const ref = tip.arxiv ? `arXiv:${tip.arxiv}` : tip.doi ? `doi:${tip.doi}` : undefined;
  if (!paper && !ref) return null;
  const url = paper?.url ?? (tip.arxiv ? `https://arxiv.org/abs/${tip.arxiv}` : tip.doi ? `https://doi.org/${tip.doi}` : undefined);
  const link = el('button', { className: 'detail-paper-ref link-button', type: 'button', text: ref ?? 'paper' });
  if (url) link.addEventListener('click', () => onOpenLink(url));
  const meta = paper ? [paper.authors.join(', '), paper.venue, paper.year].filter(Boolean).join(' · ') : t('paperNotFetched');
  return el(
    'section',
    { className: 'detail-paper' },
    paper ? el('p', { className: 'detail-paper-title', text: paper.title }) : null,
    el('p', { className: 'detail-paper-meta', text: meta }),
    link,
  );
}

/** Swatches offered for the colour override: the tile palette plus a few extras. */
export const SWATCHES: readonly string[] = [
  KIND_COLORS.task,
  KIND_COLORS.event,
  KIND_COLORS.note,
  KIND_COLORS.reading,
  DEADLINE_COLORS.later,
  DEADLINE_COLORS.soon,
  DEADLINE_COLORS.critical,
  OVERDUE_COLOR,
  '#2F4A7A', // navy
  '#3A6EA5', // sky
  '#2B5876', // teal blue
  '#2A6B6B', // sea
  '#2F5D3A', // forest
  '#556B2F', // moss
  '#8A6A1F', // ochre
  '#8A4E2A', // rust
  '#7A3B2E', // brick
  '#8A3A4A', // rose
  '#7A2E5A', // magenta
  '#6B3A5B', // plum
  '#5E3A87', // violet
  '#3B3F7A', // indigo
  '#4A5A6A', // steel
  '#5A5A2E', // olive
  '#3C3C40', // charcoal
  '#141416', // black
  '#D9D5CC', // bone (dark text)
  '#B9A88F', // sand (dark text)
];

const SIZE_LABEL: Record<TileSize, 'sizeSm' | 'sizeMd' | 'sizeWide'> = { sm: 'sizeSm', md: 'sizeMd', wide: 'sizeWide' };

/** Size and colour controls for the tile itself. */
function tileSection(tip: Tip, isNextUp: boolean, now: Date, actions: DetailActions): HTMLElement {
  const chip = (label: string, on: boolean, onClick: () => void, extra = ''): HTMLButtonElement => {
    const b = el('button', { className: `chip${on ? ' chip-on' : ''} ${extra}`.trim(), type: 'button', text: label, 'aria-pressed': on ? 'true' : 'false' });
    b.addEventListener('click', onClick);
    return b;
  };
  const explicitSize = TILE_SIZES.find((s) => s === tip.size);
  const effective = tileSize(tip, isNextUp, now);
  const sizes = el(
    'div',
    { className: 'chip-row', role: 'group', 'aria-label': t('size') },
    chip(`${t('auto')} (${t(SIZE_LABEL[effective])})`, !explicitSize, () => actions.onSetSize(tip.id, null)),
    ...TILE_SIZES.map((s) => chip(t(SIZE_LABEL[s]), explicitSize === s, () => actions.onSetSize(tip.id, s))),
  );
  const current = (tip.color ?? '').toLowerCase();
  const inPalette = SWATCHES.some((hex) => hex.toLowerCase() === current);
  // Native colour picker for anything the palette lacks.
  const picker = el('input', { type: 'color', className: 'swatch swatch-custom', 'aria-label': t('customColour'), title: t('customColour'), value: current || '#4a3b6b' });
  if (current && !inPalette) picker.classList.add('chip-on');
  picker.addEventListener('change', () => actions.onSetColor(tip.id, picker.value));
  const swatches = el(
    'div',
    { className: 'chip-row', role: 'group', 'aria-label': t('colour') },
    chip(t('auto'), !current, () => actions.onSetColor(tip.id, null)),
    ...SWATCHES.map((hex) => {
      const b = chip('', current === hex.toLowerCase(), () => actions.onSetColor(tip.id, hex), 'swatch');
      b.style.setProperty('--swatch', hex);
      b.setAttribute('aria-label', hex);
      b.title = hex;
      return b;
    }),
    picker,
  );
  return el(
    'section',
    { className: 'detail-tile' },
    el('h2', { className: 'detail-section-title', text: t('tile') }),
    el('div', { className: 'detail-tile-row' }, el('span', { className: 'detail-meta-key', text: t('size') }), sizes),
    el('div', { className: 'detail-tile-row' }, el('span', { className: 'detail-meta-key', text: t('colour') }), swatches),
  );
}

function figureName(src: string): string {
  const last = src.split(/[\\/]/).pop() ?? src;
  return last.replace(/\?.*$/, '');
}

/** Figures declared in the `images` frontmatter key, as a strip of thumbnails. */
function figures(tip: Tip, resolve: ImageResolver): HTMLElement | null {
  const sources = (tip.images ?? []).map((s) => s.trim()).filter(Boolean);
  if (!sources.length) return null;
  const items = sources.map((src, index) => {
    const name = figureName(src);
    const img = el('img', { className: 'detail-figure-img', alt: name, draggable: 'false' });
    const button = el('button', { className: 'detail-figure', type: 'button', title: name }, img, el('span', { className: 'detail-figure-caption', text: name }));
    button.style.setProperty('--i', String(index));
    const load = isRelativeSrc(src) ? resolve(src) : Promise.resolve(src);
    load
      .then((url) => {
        img.src = url;
        button.classList.add('detail-figure-ready');
        button.addEventListener('click', () => showLightbox(url, name));
      })
      .catch((error) => {
        button.classList.add('detail-figure-error');
        button.title = `${name}: ${error instanceof Error ? error.message : String(error)}`;
      });
    return el('li', {}, button);
  });
  return el(
    'section',
    { className: 'detail-figures' },
    el('h2', { className: 'detail-section-title', text: t('figures') }),
    el('ul', { className: 'detail-figure-list' }, ...items),
  );
}

function linksList(tip: Tip, onOpenLink: (url: string) => void): HTMLElement | null {
  if (!tip.links?.length) return null;
  const items = tip.links.map((url) => {
    const button = el('button', { className: 'link-button', type: 'button', text: url.replace(/^https?:\/\//, '') });
    button.addEventListener('click', () => onOpenLink(url));
    return el('li', {}, button);
  });
  return el('ul', { className: 'detail-links' }, ...items);
}

function body(tip: Tip, resolveImage: ImageResolver): HTMLElement {
  const node = el('article', { className: 'detail-body markdown' });
  node.innerHTML = renderMarkdown(tip.body);
  void resolveImages(node, resolveImage);
  // Body images open the same viewer as the figure strip.
  node.querySelectorAll('img').forEach((img) => {
    img.addEventListener('click', () => {
      if (img.src) showLightbox(img.src, img.alt || figureName(img.getAttribute('data-original-src') ?? img.src));
    });
  });
  return node;
}

function actionBar(tip: Tip, now: Date, actions: DetailActions, panel: HTMLElement): HTMLElement {
  const button = (text: string, onClick: () => void, extra = ''): HTMLButtonElement => {
    const b = el('button', { className: `action ${extra}`.trim(), type: 'button', text });
    b.addEventListener('click', onClick);
    return b;
  };
  const done = (): void => {
    panel.querySelectorAll<HTMLButtonElement>('.action').forEach((b) => (b.disabled = true));
    void animate(panel, 'detail-leaving', DONE_MS).then(() => actions.onDone(tip.id));
  };
  const primary =
    tip.status === 'done'
      ? button(t('reopen'), () => actions.onReopen(tip.id), 'action-primary')
      : button(t('done'), done, 'action-primary');
  // Delete arms on the first click and fires on the second.
  let armed: ReturnType<typeof setTimeout> | undefined;
  const del = button(t('delete'), () => {
    if (armed) {
      clearTimeout(armed);
      armed = undefined;
      panel.querySelectorAll<HTMLButtonElement>('.action').forEach((b) => (b.disabled = true));
      void animate(panel, 'detail-leaving', DONE_MS).then(() => actions.onDelete(tip.id));
      return;
    }
    del.textContent = t('deleteConfirm');
    del.classList.add('action-danger');
    armed = setTimeout(() => {
      armed = undefined;
      del.textContent = t('delete');
      del.classList.remove('action-danger');
    }, CONFIRM_MS);
  });
  return el(
    'footer',
    { className: 'detail-actions' },
    primary,
    tip.status === 'open' ? button(t('snooze1h'), () => actions.onSnooze(tip.id, SNOOZE_HOUR_MINUTES)) : null,
    tip.status === 'open' ? button(t('tomorrow'), () => actions.onSnooze(tip.id, minutesUntilTomorrowMorning(now))) : null,
    button(t('editLocal'), () => actions.onEditLocal(tip.id)),
    button(t('showInFolder'), () => actions.onReveal(tip.id)),
    del,
    button(t('back'), actions.onBack, 'action-back'),
  );
}

/** Render the flipped-open detail panel for one tip. */
export function renderDetail(options: DetailOptions): HTMLElement {
  const { tip, now, actions, isNextUp = false } = options;
  const colour = tileColor(tip, now);
  const panel = el('section', { className: 'detail', role: 'dialog', 'aria-modal': 'true', 'aria-label': tip.title });
  panel.append(
    header(tip, now),
    ...[
      metaRows(tip),
      paperBlock(tip, actions.onOpenLink),
      figures(tip, actions.resolveImage),
      body(tip, actions.resolveImage),
      linksList(tip, actions.onOpenLink),
      tileSection(tip, isNextUp, now, actions),
    ].filter(
      (n): n is HTMLElement => !!n,
    ),
    actionBar(tip, now, actions, panel),
  );
  panel.style.setProperty('--tile-bg', colour.bg);
  panel.style.setProperty('--tile-fg', colour.fg);
  return panel;
}
