import { describe, expect, it } from 'vitest';
import { CAPTURE_HINT, describeCapture } from './capture-hint';

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
