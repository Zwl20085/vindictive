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
