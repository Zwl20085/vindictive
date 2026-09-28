import { describe, expect, it } from 'vitest';
import { editUrl, fileUrl, isConfigured, newFileUrl, repoUrl } from './github';

const ref = { owner: 'Zwl20085', repo: 'vindictive-tips', branch: 'main' };

describe('github urls', () => {
  it('builds repo, file, edit and new urls', () => {
    expect(repoUrl(ref)).toBe('https://github.com/Zwl20085/vindictive-tips');
    expect(fileUrl(ref, 'tips/2026 camera.md')).toBe(
      'https://github.com/Zwl20085/vindictive-tips/blob/main/tips/2026%20camera.md',
    );
    expect(editUrl(ref, 'tips/a.md')).toBe('https://github.com/Zwl20085/vindictive-tips/edit/main/tips/a.md');
    expect(newFileUrl(ref, 'tips')).toBe('https://github.com/Zwl20085/vindictive-tips/new/main/tips');
    expect(newFileUrl(ref, '')).toBe('https://github.com/Zwl20085/vindictive-tips/new/main');
  });
  it('defaults the branch to main', () => {
    expect(fileUrl({ ...ref, branch: ' ' }, 'a.md')).toContain('/blob/main/');
  });
  it('knows when unconfigured', () => {
    expect(isConfigured(ref)).toBe(true);
    expect(isConfigured({ owner: '', repo: 'x' })).toBe(false);
  });
});
