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
    onOpenResults: vi.fn(),
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

describe('highlights in the detail', () => {
  function renderWith(t: Tip, words: string[]): HTMLElement {
    const panel = renderDetail({ tip: t, settings: { ...SAMPLE_SETTINGS, highlight_words: words }, now: NOW, actions: actions() });
    document.body.append(panel);
    return panel;
  }
  const marks = (node: Element | null): string[] =>
    Array.from(node?.querySelectorAll('mark') ?? []).map((m) => `${m.className}|${m.textContent}`);

  it('marks keywords and automatic patterns in the body, after sanitising', () => {
    const panel = renderWith(
      tip({ highlight: ['thesis'], body: 'TODO: send the Thesis to Ana by 2026-11-04 #grant\n\n`TODO in code`\n\n<img src=x onerror="alert(1)">' }),
      ['Ana'],
    );
    const body = panel.querySelector('.detail-body');
    expect(marks(body)).toEqual(['hl hl-urgent|TODO', 'hl hl-kw|Thesis', 'hl hl-kw|Ana', 'hl hl-date|2026-11-04', 'hl hl-tag|#grant']);
    expect(body?.querySelector('code mark')).toBeNull();
    expect(body?.querySelector('[onerror]')).toBeNull();
  });

  it('marks only keywords in the title and location', () => {
    const panel = renderWith(tip({ title: 'TODO thesis 2026-11-04', location: 'Room 2.14, TODO', highlight: ['thesis', 'room'] }), []);
    expect(marks(panel.querySelector('.detail-title'))).toEqual(['hl hl-kw|thesis']);
    expect(marks(panel.querySelector('.detail-meta'))).toEqual(['hl hl-kw|Room']);
    expect(panel.querySelector('.detail-title-text')?.textContent).toBe('TODO thesis 2026-11-04');
  });

  it('does not mark a link URL inside an anchor', () => {
    const panel = renderWith(tip({ body: 'See <https://example.org/p> and https://example.org/q' }), []);
    expect(panel.querySelectorAll('.detail-body a[href]').length).toBeGreaterThan(0);
    expect(panel.querySelector('.detail-body a mark.hl-url')).toBeNull();
  });
});

describe('agent detail', () => {
  const agentTip = tip({
    id: 'survey',
    path: 'survey.md',
    title: 'Survey hairpin AC loss papers',
    kind: 'agent',
    body: 'Prompt\n',
    agent: { v: 1, state: 'done', hash: 'sha256:x', host: 'DESKTOP-ABC', at: '2026-11-09T19:31:00', results: 'agent/survey/' },
    agent_state: 'done',
  });

  it('shows the state in words, the host and the time', () => {
    const block = render(agentTip).panel.querySelector('.detail-agent');
    expect(block?.classList.contains('detail-agent-done')).toBe(true);
    const text = block?.textContent ?? '';
    expect(text).toContain('⭐ Agent finished');
    expect(text).toContain('DESKTOP-ABC');
    expect(text).toContain('19:31');
  });

  it('offers Open results only when results are recorded', () => {
    const { panel, a } = render(agentTip);
    const button = panel.querySelector<HTMLButtonElement>('.detail-agent-results');
    expect(button?.textContent).toBe('Open results');
    button?.click();
    expect(a.onOpenResults).toHaveBeenCalledWith('survey');
    const queued = render({ ...agentTip, agent: undefined, agent_state: 'queued' }).panel;
    expect(queued.querySelector('.detail-agent-results')).toBeNull();
    expect(queued.querySelector('.detail-agent')?.textContent).toContain('Waiting for the agent');
  });

  it('non-agent tips have no agent block', () => {
    expect(render({ ...agentTip, kind: 'task', agent_state: undefined }).panel.querySelector('.detail-agent')).toBeNull();
  });
});

describe('question detail', () => {
  const question = (extra: Partial<Tip> = {}): Tip => tip({ id: 'questions/a', path: 'questions/a.md', ...extra });

  it('says it is a question in the header, in words', () => {
    const { panel } = render(question());
    const kind = panel.querySelector('.detail-kind');
    expect(kind?.textContent).toContain('Question');
    const badge = kind?.querySelector('.detail-question-badge');
    expect(badge?.textContent).toBe('❓');
    expect(badge?.getAttribute('aria-hidden')).toBe('true');
  });

  it('shows no agent block, even for kind agent with an agent map', () => {
    const { panel } = render(question({ kind: 'agent', agent: { v: 1, state: 'done', host: 'PC', results: 'agent/a/' } }));
    expect(panel.querySelector('.detail-agent')).toBeNull();
    expect(panel.querySelector('.detail-agent-results')).toBeNull();
    expect(panel.querySelector('.detail-kind')?.textContent).not.toContain('Agent');
  });

  it('keeps Edit and Done, like any tip', async () => {
    const { panel, a } = render(question());
    const buttons = Array.from(panel.querySelectorAll<HTMLButtonElement>('.detail-actions button'));
    buttons.find((b) => b.textContent === 'Edit')?.click();
    expect(a.onEdit).toHaveBeenCalledWith('questions/a');
    buttons.find((b) => b.textContent === 'Done')?.click();
    await vi.waitFor(() => expect(a.onDone).toHaveBeenCalledWith('questions/a'));
  });
});
