import { describe, expect, it } from 'vitest';
import { editUrl, fileUrl, isConfigured, newFileUrl, newRepoUrl, newTokenUrl, parseRepoInput, repoUrl } from './github';

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

describe('parseRepoInput', () => {
  const expected = { owner: 'Zwl20085', repo: 'vindictive-tips' };
  it.each([
    'Zwl20085/vindictive-tips',
    ' Zwl20085/vindictive-tips/ ',
    'github.com/Zwl20085/vindictive-tips',
    'https://github.com/Zwl20085/vindictive-tips',
    'https://github.com/Zwl20085/vindictive-tips.git',
    'https://github.com/Zwl20085/vindictive-tips/tree/main/tips',
    'git@github.com:Zwl20085/vindictive-tips.git',
  ])('reads %s', (input) => {
    expect(parseRepoInput(input)).toEqual(expected);
  });
  it('rejects plain names and other hosts', () => {
    expect(parseRepoInput('Zwl20085')).toBeUndefined();
    expect(parseRepoInput('https://gitlab.com/a/b')).toBeUndefined();
    expect(parseRepoInput('')).toBeUndefined();
  });
});

describe('setup links', () => {
  it('pre-fills a private repo', () => {
    const url = new URL(newRepoUrl());
    expect(url.pathname).toBe('/new');
    expect(url.searchParams.get('name')).toBe('vindictive-tips');
    expect(url.searchParams.get('visibility')).toBe('private');
  });
  it('pre-fills a contents:write fine-grained token', () => {
    const url = new URL(newTokenUrl('Zwl20085'));
    expect(url.pathname).toBe('/settings/personal-access-tokens/new');
    expect(url.searchParams.get('contents')).toBe('write');
    expect(url.searchParams.get('target_name')).toBe('Zwl20085');
    expect(new URL(newTokenUrl()).searchParams.has('target_name')).toBe(false);
  });
});
