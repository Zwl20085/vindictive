import { afterEach, describe, expect, it } from 'vitest';
import type { AgentState, Tip } from '../types';
import { AGENT_BADGE, agentInfo, agentStateLabel, bodyHash, displayState, normaliseBody, sha256Hex, withAgentState } from './agent';
import { kindLabel, setLocale } from './i18n';
import { foregroundFor, KIND_COLORS, LIGHT_FG, tileColor } from './tileColor';
import { tileSize } from './tileSize';
import { renderMarkdown } from './markdown';

afterEach(() => setLocale('en'));

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

const STATES: AgentState[] = ['queued', 'running', 'done', 'question', 'failed'];

describe('agent badges', () => {
  it('uses the protocol emoji for every state', () => {
    expect(AGENT_BADGE).toEqual({ queued: '🕓', running: '⚙', done: '⭐', question: '❓', failed: '⚠' });
  });
  it('has words for every state in every language', () => {
    for (const lang of ['en', 'zh'] as const) {
      setLocale(lang);
      const labels = STATES.map(agentStateLabel);
      expect(new Set(labels).size).toBe(STATES.length);
      labels.forEach((l) => expect(l).not.toMatch(/^agent_/));
    }
    setLocale('en');
    expect(agentStateLabel('done')).toBe('Agent finished');
    expect(agentStateLabel('question')).toBe('Agent has a question');
    expect(kindLabel('agent', 'en')).toBe('Agent');
    expect(kindLabel('agent', 'zh')).toBe('智能体');
  });
});

describe('agentInfo', () => {
  it('reads string sub-keys of a well-formed map', () => {
    const info = agentInfo(tip({ agent: { v: 1, state: 'done', host: 'DESK', at: '2026-11-09T07:31:00', results: 'agent/survey/' } }));
    expect(info).toEqual({ host: 'DESK', at: '2026-11-09T07:31:00', results: 'agent/survey/' });
  });
  it('treats malformed values as absent', () => {
    for (const agent of [undefined, null, 'text', 3, ['a'], { host: 42, results: '  ', at: { x: 1 } }]) {
      expect(agentInfo(tip({ agent }))).toEqual({ host: undefined, at: undefined, results: undefined });
    }
  });
});

describe('agent tiles', () => {
  it('have their own kind colour with light text', () => {
    expect(tileColor(tip({}), now).bg).toBe(KIND_COLORS.agent);
    expect(foregroundFor(KIND_COLORS.agent)).toBe(LIGHT_FG);
  });
  it('are at least md while the agent has a question', () => {
    expect(tileSize(tip({ agent_state: 'question' }), false, now)).toBe('md');
    expect(tileSize(tip({ agent_state: 'question' }), true, now)).toBe('wide');
    expect(tileSize(tip({ agent_state: 'question', size: 'sm' }), false, now)).toBe('sm');
    expect(tileSize(tip({ agent_state: 'done' }), false, now)).toBe('sm');
    expect(tileSize(tip({ agent_state: 'queued' }), false, now)).toBe('sm');
  });
});

describe('agent blocks in the body', () => {
  it('do not show the begin / end comment lines as text', () => {
    const body =
      'Prompt\n\n<!-- agent:begin 2026-11-09T07:31:00 question -->\n## ❓ Questions · 2026-11-09 07:31\n1. Which year range?\n<!-- agent:end -->\n\n2020 onwards\n';
    const html = renderMarkdown(body);
    const box = document.createElement('div');
    box.innerHTML = html;
    expect(box.textContent).not.toContain('agent:begin');
    expect(box.textContent).not.toContain('agent:end');
    expect(box.querySelector('h2')?.textContent).toBe('❓ Questions · 2026-11-09 07:31');
    expect(box.textContent).toContain('2020 onwards');
  });
});

describe('body hash (protocol section 4)', () => {
  const hex = (body: string): string => bodyHash(body).replace(/^sha256:/, '');

  it('matches the protocol test vectors', () => {
    expect(hex('Find three recent papers on hairpin AC loss.\r\n\r\n')).toBe('a7b5da43b6fbfa12a04b1d93b1c7eb5b224f672554632f78c5bf68c6f5a2386f');
    expect(hex('\u{feff}\n  line one  \r\n\tline two\t\n\n\n')).toBe('7ceac30c0680c7b43ef2cc5971ddff12cd7584a56cf6fa615f865ffb6f97adca');
    const empty = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
    expect(hex('')).toBe(empty);
    expect(hex('\n \n')).toBe(empty);
    expect(hex('调研 ⭐\n')).toBe('2115d91d771e0eafc845500cc950c60545e93cd145b2f4b80434fd36a6bf9f95');
    expect(
      hex('Prompt\n\n<!-- agent:begin 2026-11-09T07:31:00 question -->\n## ❓ Questions\n1. Which year range?\n<!-- agent:end -->\n\n2020 onwards\n'),
    ).toBe('262828d169635feb78f04a5a63dde71f681a21c534598a9b19556af76446b9ac');
    expect(bodyHash('x')).toMatch(/^sha256:[0-9a-f]{64}$/);
  });

  it('pads correctly around the 64-byte block boundaries', () => {
    const want: Record<number, string> = {
      55: '9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318',
      56: 'b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a',
      63: '7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34',
      64: 'ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb',
      119: '31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb',
      120: '2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c',
    };
    for (const [n, hex] of Object.entries(want)) {
      expect(sha256Hex(new TextEncoder().encode('a'.repeat(Number(n)))), n).toBe(hex);
    }
  });

  it('normalises like core/agent.rs', () => {
    expect(normaliseBody('\u{feff}a\rb\r\n c \t\n')).toBe('a\nb\n c');
    expect(normaliseBody('\n\n  x\n\n\ny  \n\n')).toBe('  x\n\n\ny');
    expect(normaliseBody(' \n\t\n')).toBe('');
  });
});

describe('displayState (protocol section 5)', () => {
  const H = 'sha256:h';
  const st = (agent: unknown, kind: Tip['kind'] = 'agent') => displayState({ kind, agent }, H);

  it('follows the table, first match wins', () => {
    expect(st({ state: 'done', hash: H }, 'task')).toBeUndefined();
    expect(st({ v: 2, state: 'done', hash: H })).toBeUndefined();
    expect(st({ state: 'running', hash: 'other' })).toBe('running');
    expect(st(undefined)).toBe('queued');
    expect(st({ hash: H })).toBe('queued');
    expect(st({ state: 'done' })).toBe('queued');
    expect(st({ state: 'done', hash: 'sha256:stale' })).toBe('queued');
    expect(st({ v: 1, state: 'done', hash: H })).toBe('done');
    expect(st({ state: 'question', hash: H })).toBe('question');
    expect(st({ state: 'failed', hash: H })).toBe('failed');
    expect(st({ state: 'weird', hash: H })).toBe('queued');
    expect(st(['not', 'a', 'map'])).toBe('queued');
    expect(st('text')).toBe('queued');
  });

  it('withAgentState derives the state from the body and keeps the map', () => {
    const body = 'Prompt\n';
    const agent = { v: 1, state: 'done', hash: bodyHash(body), zz: 'keep' };
    const done = withAgentState(tip({ body, agent }));
    expect(done.agent_state).toBe('done');
    expect(done.agent).toBe(agent);
    expect((withAgentState(tip({ body: 'Prompt\n\nmore', agent }))).agent_state).toBe('queued');
    expect((withAgentState(tip({ kind: 'task', body, agent, agent_state: 'done' }))).agent_state).toBeUndefined();
  });
});
