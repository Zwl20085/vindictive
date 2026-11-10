import { describe, expect, it } from 'vitest';
import { MAX_DERIVED_TITLE_CHARS, titleFromPrompt } from './prompt';
import vectors from './prompt-vectors.json';

/** Shared with `src-tauri/src/core/prompt.rs`, which reads the same file. */
const VECTORS = vectors as [string, string | null][];

describe('titleFromPrompt', () => {
  it('runs every shared vector', () => {
    expect(VECTORS.length).toBeGreaterThanOrEqual(25);
  });
  it.each(VECTORS)('%j', (prompt, want) => {
    expect(titleFromPrompt(prompt)).toBe(want ?? undefined);
  });
  it('keeps derived titles short', () => {
    const title = titleFromPrompt('word '.repeat(100)) ?? '';
    expect(Array.from(title).length).toBeLessThanOrEqual(MAX_DERIVED_TITLE_CHARS + 1);
    expect(title.endsWith('…')).toBe(true);
  });
});
