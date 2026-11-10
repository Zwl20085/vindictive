import { describe, expect, it, vi } from 'vitest';
import type { AgentState, Tip } from '../types';
import { renderTile } from './tile';

const now = new Date(2026, 10, 9, 12, 0, 0);

function tip(over: Partial<Tip>): Tip {
  return {
    id: 'survey',
    path: 'survey.md',
    title: 'Survey hairpin AC loss papers',
    kind: 'agent',
    priority: 'normal',
    status: 'open',
    body: '',
    remind_at: [],
    ...over,
  };
}

function render(t: Tip): HTMLButtonElement {
  return renderTile({ tip: t, isNextUp: false, now, columns: 4, onOpen: vi.fn() });
}

describe('agent tile badge', () => {
  const cases: [AgentState, string, string][] = [
    ['queued', '🕓', 'Waiting for the agent'],
    ['running', '⚙', 'Agent running'],
    ['done', '⭐', 'Agent finished'],
    ['question', '❓', 'Agent has a question'],
    ['failed', '⚠', 'Agent failed'],
  ];
  it.each(cases)('%s shows %s with words', (state, emoji, words) => {
    const tile = render(tip({ agent_state: state }));
    const badge = tile.querySelector('.tile-top .tile-agent-badge');
    expect(badge?.textContent).toBe(emoji);
    expect(badge?.getAttribute('aria-label')).toBe(words);
    expect(badge?.getAttribute('title')).toBe(words);
    expect(tile.classList.contains(`tile-agent-${state}`)).toBe(true);
    expect(tile.getAttribute('aria-label')).toContain(words);
  });

  it('a question makes the tile md', () => {
    expect(render(tip({ agent_state: 'question' })).classList.contains('tile-md')).toBe(true);
    expect(render(tip({ agent_state: 'done' })).classList.contains('tile-sm')).toBe(true);
  });

  it('tips without an agent state have no badge', () => {
    const plain = render(tip({ kind: 'task' }));
    expect(plain.querySelector('.tile-agent-badge')).toBeNull();
    expect(Array.from(plain.classList).some((c) => c.startsWith('tile-agent-'))).toBe(false);
    // An agent tip from a newer protocol shows as a plain agent tile.
    expect(render(tip({ agent: { v: 2 } })).querySelector('.tile-agent-badge')).toBeNull();
  });
});

describe('question tile', () => {
  const question = (over: Partial<Tip> = {}): Tip => tip({ id: 'questions/mesh', path: 'questions/mesh.md', title: 'Pick a mesh', kind: 'note', ...over });

  it('shows ❓ with the word for screen readers and hover', () => {
    const tile = render(question());
    const badge = tile.querySelector('.tile-top .tile-question-badge');
    expect(badge?.textContent).toBe('❓');
    expect(badge?.getAttribute('role')).toBe('img');
    expect(badge?.getAttribute('aria-label')).toBe('Question');
    expect(badge?.getAttribute('title')).toBe('Question');
    expect(tile.classList.contains('tile-question')).toBe(true);
    expect(tile.getAttribute('aria-label')).toMatch(/^Question, /);
    expect(tile.dataset.id).toBe('questions/mesh');
  });

  it('kind agent shows as a plain question: no agent badge, glyph or label', () => {
    const tile = render(question({ kind: 'agent', agent: { v: 1, state: 'question' } }));
    expect(tile.querySelector('.tile-agent-badge')).toBeNull();
    expect(Array.from(tile.classList).some((c) => c.startsWith('tile-agent-'))).toBe(false);
    expect(tile.querySelector('.tile-glyph')?.textContent).toBe('≡');
    expect(tile.getAttribute('aria-label')).not.toContain('Agent');
    expect(tile.querySelector('.tile-question-badge')).not.toBeNull();
  });

  it('main tips have no question badge', () => {
    expect(render(tip({ kind: 'note' })).querySelector('.tile-question-badge')).toBeNull();
  });
});
