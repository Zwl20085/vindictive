import { describe, expect, it } from 'vitest';
import { CAPTURE_HINT, describeCapture, parseCapture, resolveCaptureDate } from './capture-hint';

describe('describeCapture', () => {
  it('summarises the full syntax', () => {
    expect(describeCapture('Check coil temp #lab #thermal !high @tomorrow 09:30 ^"Lab 302"')).toBe(
      'Check coil temp · #lab #thermal · high · due tomorrow 09:30 · @ Lab 302',
    );
  });
  it('handles kind, absolute date and papers', () => {
    expect(describeCapture('>event Group meeting @2026-10-05 10:00')).toBe(
      'Group meeting · event · due 2026-10-05 10:00',
    );
    expect(describeCapture('https://arxiv.org/abs/2401.12345v2')).toBe('arXiv 2401.12345');
    expect(describeCapture('Read this 10.1109/TIE.2024.1234567')).toBe(
      'Read this · DOI 10.1109/TIE.2024.1234567',
    );
  });
  it('keeps unknown markers in the title', () => {
    expect(describeCapture('Ask about !budget @noon >thing')).toBe('Ask about !budget @noon >thing');
    expect(describeCapture('!low quick')).toBe('quick · low');
  });
  it('is empty for empty input', () => {
    expect(describeCapture('   ')).toBe('');
    expect(CAPTURE_HINT).toContain('#tag');
  });
});

describe('parseCapture', () => {
  const now = new Date(2026, 9, 14, 12, 0, 0); // Wed 14 Oct 2026

  it('reads every marker into form fields', () => {
    expect(parseCapture('Check coil temp #lab #thermal !high @tomorrow 9:30 ^"Lab 302" >deadline', now)).toEqual({
      title: 'Check coil temp',
      tags: ['lab', 'thermal'],
      priority: 'high',
      due: '2026-10-15 09:30',
      location: 'Lab 302',
      kind: 'deadline',
    });
  });
  it('resolves date words and absolute dates, with or without a time', () => {
    expect(parseCapture('a @today', now).due).toBe('2026-10-14');
    expect(parseCapture('a @tod', now).due).toBe('2026-10-14');
    expect(parseCapture('a @tmr', now).due).toBe('2026-10-15');
    expect(parseCapture('a @nextweek', now).due).toBe('2026-10-21');
    expect(parseCapture('a @2026/11/02 18:00', now).due).toBe('2026-11-02 18:00');
    expect(parseCapture('a @2026-11-02', now).due).toBe('2026-11-02');
  });
  it('keeps an impossible date, a stray time and unknown markers in the title', () => {
    expect(parseCapture('a @2026-02-30', now)).toEqual({ title: 'a @2026-02-30' });
    expect(parseCapture('call at 09:30', now)).toEqual({ title: 'call at 09:30' });
    expect(parseCapture('a @today 25:00', now)).toEqual({ title: 'a 25:00', due: '2026-10-14' });
    expect(parseCapture('Ask about !budget @noon >thing', now)).toEqual({ title: 'Ask about !budget @noon >thing' });
  });
  it('maps kind synonyms and low priority; full-width markers count', () => {
    expect(parseCapture('Group >meeting !l', now)).toEqual({ title: 'Group', kind: 'event', priority: 'low' });
    expect(parseCapture('x >paper', now).kind).toBe('reading');
    expect(parseCapture('x >due', now).kind).toBe('deadline');
    // The add bar prefills the new-tip form from this: an agent tip, no `agent` map.
    expect(parseCapture('Survey AC loss >agent', now)).toEqual({ title: 'Survey AC loss', kind: 'agent' });
    expect(parseCapture('Submit ＠2026-11-02 ！low ＃论文', now)).toEqual({ title: 'Submit', due: '2026-11-02', priority: 'low', tags: ['论文'] });
  });
  it('leaves arXiv ids and DOIs in the title', () => {
    expect(parseCapture('Read https://arxiv.org/abs/2401.12345v2', now).title).toBe('Read https://arxiv.org/abs/2401.12345v2');
    expect(parseCapture('Read this 10.1109/TIE.2024.1234567 #lit', now)).toEqual({ title: 'Read this 10.1109/TIE.2024.1234567', tags: ['lit'] });
  });
  it('deduplicates tags and gives an empty title for marker-only input', () => {
    expect(parseCapture('#a #a !high', now)).toEqual({ title: '', tags: ['a'], priority: 'high' });
    expect(parseCapture('   ', now)).toEqual({ title: '' });
  });
});

describe('resolveCaptureDate', () => {
  it('rejects shapes that are not real dates', () => {
    const now = new Date(2026, 0, 31);
    expect(resolveCaptureDate('tomorrow', now)).toBe('2026-02-01');
    expect(resolveCaptureDate('2026-13-01', now)).toBeUndefined();
    expect(resolveCaptureDate('noon', now)).toBeUndefined();
  });
});
