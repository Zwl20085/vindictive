import { describe, expect, it } from 'vitest';
import type { Tip } from '../types';
import { withAgentState } from './agent';
import { tileColor, KIND_COLORS } from './tileColor';
import { tileSize } from './tileSize';
import { displayKind, isQuestion, isQuestionPath, QUESTION_BADGE } from './question';

const now = new Date(2026, 10, 10, 12, 0, 0);

function tip(over: Partial<Tip>): Tip {
  return { id: 'questions/q', path: 'questions/q.md', title: 'Pick a mesh', kind: 'note', priority: 'normal', status: 'open', body: '', remind_at: [], ...over };
}

describe('question tips', () => {
  it('are the files directly in questions/', () => {
    expect(isQuestionPath('questions/q.md')).toBe(true);
    for (const no of ['q.md', 'questions/', 'questions/deeper/q.md', 'questions\\q.md', 'questionsq.md', 'agent/questions/q.md']) {
      expect(isQuestionPath(no), no).toBe(false);
    }
    expect(isQuestion(tip({}))).toBe(true);
    expect(isQuestion(tip({ id: 'q', path: 'q.md' }))).toBe(false);
    expect(QUESTION_BADGE).toBe('❓');
  });

  it('show kind agent as a plain note, other kinds as they are', () => {
    expect(displayKind(tip({ kind: 'agent' }))).toBe('note');
    expect(displayKind(tip({ kind: 'task' }))).toBe('task');
    expect(displayKind(tip({ id: 'a', path: 'a.md', kind: 'agent' }))).toBe('agent');
    expect(tileColor(tip({ kind: 'agent' }), now).bg).toBe(KIND_COLORS.note);
  });

  it('never get an agent state, whatever the frontmatter says', () => {
    const asked = tip({ kind: 'agent', agent: { v: 1, state: 'running' } });
    expect(withAgentState(asked).agent_state).toBeUndefined();
    expect(withAgentState({ ...asked, id: 'q', path: 'q.md' }).agent_state).toBe('running');
  });

  it('are md while open, like an agent question', () => {
    expect(tileSize(tip({}), false, now)).toBe('md');
    expect(tileSize(tip({ size: 'sm' }), false, now)).toBe('sm');
    expect(tileSize(tip({ status: 'done' }), false, now)).toBe('sm');
    expect(tileSize(tip({ id: 'q', path: 'q.md' }), false, now)).toBe('sm');
  });
});
