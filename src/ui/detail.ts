import type { Settings, Tip } from '../types';
import { countdown, formatLocal, minutesUntilTomorrowMorning, parseNaive } from '../lib/time';
import { tileColor } from '../lib/tileColor';
import { renderMarkdown, resolveImages, type ImageResolver } from '../lib/markdown';
import { editUrl } from '../lib/github';
import { el } from './dom';
import { KIND_GLYPH, KIND_LABEL } from './glyphs';

export const SNOOZE_HOUR_MINUTES = 60;

export interface DetailActions {
  onDone: (id: string) => void;
  onReopen: (id: string) => void;
  onSnooze: (id: string, minutes: number) => void;
  onOpenLink: (url: string) => void;
  onBack: () => void;
  resolveImage: ImageResolver;
}

export interface DetailOptions {
  tip: Tip;
  settings: Settings;
  now: Date;
  actions: DetailActions;
}

function header(tip: Tip, now: Date): HTMLElement {
  const due = parseNaive(tip.due_at);
  const sub = due ? `${countdown(due, now)} · ${formatLocal(due)}` : KIND_LABEL[tip.kind];
  return el(
    'header',
    { className: 'detail-header' },
    el('span', { className: 'detail-kind' }, el('span', { 'aria-hidden': 'true', text: KIND_GLYPH[tip.kind] }), ` ${KIND_LABEL[tip.kind]}`),
    el('h1', { className: 'detail-title', text: tip.title }),
    el('p', { className: 'detail-sub', text: sub }),
  );
}

function metaRows(tip: Tip): HTMLElement | null {
  const rows: HTMLElement[] = [];
  if (tip.location) rows.push(el('li', { className: 'detail-meta-row' }, el('span', { className: 'detail-meta-key', text: 'where' }), tip.location));
  if (tip.repeat) rows.push(el('li', { className: 'detail-meta-row' }, el('span', { className: 'detail-meta-key', text: 'repeat' }), tip.repeat));
  if (tip.tags?.length) {
    rows.push(el('li', { className: 'detail-meta-row' }, el('span', { className: 'detail-meta-key', text: 'tags' }), tip.tags.map((t) => `#${t}`).join(' ')));
  }
  if (tip.snoozed_until_at) {
    const until = parseNaive(tip.snoozed_until_at);
    if (until) rows.push(el('li', { className: 'detail-meta-row' }, el('span', { className: 'detail-meta-key', text: 'snoozed' }), formatLocal(until)));
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
  const meta = paper ? [paper.authors.join(', '), paper.venue, paper.year].filter(Boolean).join(' · ') : 'metadata not fetched yet';
  return el(
    'section',
    { className: 'detail-paper' },
    paper ? el('p', { className: 'detail-paper-title', text: paper.title }) : null,
    el('p', { className: 'detail-paper-meta', text: meta }),
    link,
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
  return node;
}

function actionBar(tip: Tip, settings: Settings, now: Date, actions: DetailActions): HTMLElement {
  const button = (text: string, onClick: () => void, extra = ''): HTMLButtonElement => {
    const b = el('button', { className: `action ${extra}`.trim(), type: 'button', text });
    b.addEventListener('click', onClick);
    return b;
  };
  const primary =
    tip.status === 'done'
      ? button('Reopen', () => actions.onReopen(tip.id), 'action-primary')
      : button('Done', () => actions.onDone(tip.id), 'action-primary');
  return el(
    'footer',
    { className: 'detail-actions' },
    primary,
    tip.status === 'open' ? button('Snooze 1h', () => actions.onSnooze(tip.id, SNOOZE_HOUR_MINUTES)) : null,
    tip.status === 'open' ? button('Tomorrow', () => actions.onSnooze(tip.id, minutesUntilTomorrowMorning(now))) : null,
    button('Edit on GitHub', () => actions.onOpenLink(editUrl(settings, tip.path))),
    button('Back', actions.onBack, 'action-back'),
  );
}

/** Render the flipped-open detail panel for one tip. */
export function renderDetail(options: DetailOptions): HTMLElement {
  const { tip, settings, now, actions } = options;
  const colour = tileColor(tip, now);
  const panel = el(
    'section',
    { className: 'detail', role: 'dialog', 'aria-modal': 'true', 'aria-label': tip.title },
    header(tip, now),
    metaRows(tip),
    paperBlock(tip, actions.onOpenLink),
    body(tip, actions.resolveImage),
    linksList(tip, actions.onOpenLink),
    actionBar(tip, settings, now, actions),
  );
  panel.style.setProperty('--tile-bg', colour.bg);
  panel.style.setProperty('--tile-fg', colour.fg);
  return panel;
}
