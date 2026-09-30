import type { Settings } from '../types';

export const GITHUB_BASE = 'https://github.com';

type RepoRef = Pick<Settings, 'owner' | 'repo' | 'branch'>;

function encodePath(path: string): string {
  return path
    .split('/')
    .filter((segment) => segment.length > 0)
    .map(encodeURIComponent)
    .join('/');
}

export function repoUrl(ref: Pick<Settings, 'owner' | 'repo'>): string {
  return `${GITHUB_BASE}/${encodeURIComponent(ref.owner)}/${encodeURIComponent(ref.repo)}`;
}

/** Web URL of a tip file, e.g. `https://github.com/o/r/blob/main/tips/x.md`. */
export function fileUrl(ref: RepoRef, path: string): string {
  const branch = ref.branch.trim() || 'main';
  return `${repoUrl(ref)}/blob/${encodeURIComponent(branch)}/${encodePath(path)}`;
}

/** URL of GitHub's web editor for a tip file. */
export function editUrl(ref: RepoRef, path: string): string {
  return fileUrl(ref, path).replace('/blob/', '/edit/');
}

/** URL of GitHub's "new file" page inside the tips directory. */
export function newFileUrl(ref: RepoRef, dir: string): string {
  const branch = ref.branch.trim() || 'main';
  const suffix = dir.trim() ? `/${encodePath(dir)}` : '';
  return `${repoUrl(ref)}/new/${encodeURIComponent(branch)}${suffix}`;
}

export function isConfigured(ref: Pick<Settings, 'owner' | 'repo'>): boolean {
  return ref.owner.trim().length > 0 && ref.repo.trim().length > 0;
}

/** Suggested name for a new tips repository. */
export const DEFAULT_TIPS_REPO = 'vindictive-tips';

/**
 * Split a pasted repository reference into owner and repo. Accepts
 * `owner/repo`, `github.com/owner/repo`, full https or ssh clone URLs, with or
 * without a trailing `.git`. Returns `undefined` for anything else.
 */
export function parseRepoInput(input: string): { owner: string; repo: string } | undefined {
  const cleaned = input
    .trim()
    .replace(/^git@github\.com:/i, '')
    .replace(/^(https?:\/\/)?(www\.)?github\.com\//i, '')
    .replace(/\.git$/i, '')
    .replace(/\/+$/, '');
  const match = /^([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)(?:\/.*)?$/.exec(cleaned);
  if (!match?.[1] || !match[2]) return undefined;
  return { owner: match[1], repo: match[2] };
}

/** GitHub's "new repository" page, pre-filled as a private tips repo. */
export function newRepoUrl(name: string = DEFAULT_TIPS_REPO): string {
  const params = new URLSearchParams({ name, visibility: 'private', description: 'Tips for the Vindictive board' });
  return `${GITHUB_BASE}/new?${params.toString()}`;
}

/**
 * GitHub's fine-grained token page, pre-filled with the one permission the app
 * needs. GitHub cannot pre-select the repository; the user picks it there.
 */
export function newTokenUrl(owner = ''): string {
  const params = new URLSearchParams({
    name: 'vindictive',
    description: 'Vindictive board: read and write tips',
    expires_in: '366',
    contents: 'write',
  });
  if (owner.trim()) params.set('target_name', owner.trim());
  return `${GITHUB_BASE}/settings/personal-access-tokens/new?${params.toString()}`;
}
