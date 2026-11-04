import { afterEach, describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import type { Tip } from '../types';
import { refreshDetail, renderDetail, type DetailActions } from './detail';

const NOW = new Date('2026-11-04T10:00:00');

function tip(extra: Partial<Tip> = {}): Tip {
  return { id: 'a', path: 'a.md', sha: 'sha-a', title: 'Tip a', kind: 'task', priority: 'normal', status: 'open', body: '', remind_at: [], ...extra };
}

function actions(): DetailActions {
  return {
    onDone: vi.fn(),
    onReopen: vi.fn(),
    onSnooze: vi.fn(),
    onDelete: vi.fn(),
    onEdit: vi.fn(),
    onEditLocal: vi.fn(),
    onSetColor: vi.fn(),
    onSetSize: vi.fn(),
    onOpenLink: vi.fn(),
    onReveal: vi.fn(),
    onBack: vi.fn(),
    resolveImage: vi.fn(async (src: string) => `data:${src}`),
  };
}

function render(t: Tip, a = actions()): { panel: HTMLElement; a: DetailActions } {
  const panel = renderDetail({ tip: t, settings: SAMPLE_SETTINGS, now: NOW, actions: a });
  document.body.append(panel);
  return { panel, a };
}

function clickLink(panel: HTMLElement, selector: string): MouseEvent {
  const anchor = panel.querySelector<HTMLAnchorElement>(selector);
  if (!anchor) throw new Error(`no link ${selector}`);
  const event = new MouseEvent('click', { bubbles: true, cancelable: true });
  anchor.dispatchEvent(event);
  return event;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('markdown links in the detail body', () => {
  it('opens http(s) links externally and never navigates the webview', () => {
    const { panel, a } = render(tip({ body: 'See [the paper](https://example.org/p).' }));
    const event = clickLink(panel, '.markdown a[href]');
    expect(event.defaultPrevented).toBe(true);
    expect(a.onOpenLink).toHaveBeenCalledWith('https://example.org/p');
  });

  it('handles a click on an element inside the link', () => {
    const { panel, a } = render(tip({ body: 'See [**bold**](http://example.org/b).' }));
    const strong = panel.querySelector('.markdown a strong');
    strong?.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(a.onOpenLink).toHaveBeenCalledWith('http://example.org/b');
  });

  it('swallows non-http links without opening them', () => {
    const { panel, a } = render(tip({ body: '[local](notes/other.md) and [mail](mailto:me@example.org)' }));
    expect(clickLink(panel, '.markdown a[href="notes/other.md"]').defaultPrevented).toBe(true);
    expect(clickLink(panel, '.markdown a[href^="mailto:"]').defaultPrevented).toBe(true);
    expect(a.onOpenLink).not.toHaveBeenCalled();
  });
});

describe('detail panel', () => {
  it('is focusable without being a tab stop and records its tip version', () => {
    const { panel } = render(tip());
    expect(panel.getAttribute('tabindex')).toBe('-1');
    expect(panel.dataset.sha).toBe('sha-a');
  });

  it('Done disables every action before calling onDone', async () => {
    const { panel, a } = render(tip());
    const done = panel.querySelector<HTMLButtonElement>('.action-primary');
    done?.click();
    expect(Array.from(panel.querySelectorAll<HTMLButtonElement>('.action')).every((b) => b.disabled)).toBe(true);
    await vi.waitFor(() => expect(a.onDone).toHaveBeenCalledWith('a'));
  });

  it('Delete needs a second click, then disables actions and calls onDelete', async () => {
    const { panel, a } = render(tip());
    const del = Array.from(panel.querySelectorAll<HTMLButtonElement>('.action')).find((b) => b.textContent === 'Delete');
    del?.click();
    expect(del?.disabled).toBe(false);
    del?.click();
    expect(del?.disabled).toBe(true);
    await vi.waitFor(() => expect(a.onDelete).toHaveBeenCalledWith('a'));
  });

  it('loads figures lazily, keyed by the tip sha', () => {
    const { panel, a } = render(tip({ images: ['fig/one.png'] }));
    const img = panel.querySelector('.detail-figure-img');
    expect(img?.getAttribute('loading')).toBe('lazy');
    expect(img?.getAttribute('decoding')).toBe('async');
    expect(a.resolveImage).toHaveBeenCalledWith('fig/one.png', 'sha-a');
  });

  it('refreshDetail updates the countdown in place', () => {
    const t = tip({ kind: 'deadline', due_at: '2026-11-06T10:00:00' });
    const { panel } = render(t);
    const sub = panel.querySelector('.detail-sub');
    const before = sub?.textContent;
    refreshDetail(panel, t, new Date('2026-11-05T13:00:00'));
    expect(panel.querySelector('.detail-sub')).toBe(sub);
    expect(sub?.textContent).not.toBe(before);
  });
});

describe('parse warnings in the detail', () => {
  it('lists each warning under a heading, right after the header', () => {
    const warnings = ['due: cannot parse "15/10/2026"', 'arxiv: unquoted id read as a number; quote it in the file'];
    const { panel } = render(tip({ warnings }));
    const block = panel.querySelector('.detail-warnings');
    expect(block?.previousElementSibling?.classList.contains('detail-header')).toBe(true);
    expect(block?.querySelector('.detail-section-title')?.textContent).toBe('Parse warnings');
    expect(Array.from(block?.querySelectorAll('li') ?? []).map((li) => li.textContent)).toEqual(warnings);
  });

  it('shows no block without warnings', () => {
    expect(render(tip()).panel.querySelector('.detail-warnings')).toBeNull();
    expect(render(tip({ warnings: [] })).panel.querySelector('.detail-warnings')).toBeNull();
  });
});
