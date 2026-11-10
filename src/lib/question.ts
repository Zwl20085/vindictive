/**
 * Question tips: files directly in the `questions/` subfolder of the tips
 * folder, where a Claude session leaves questions for the owner. Mirrors
 * `src-tauri/src/core/question.rs`. Being a question is derived from the
 * path, never stored; such a tip's id is `questions/<name>`.
 */
import type { Kind, Tip } from '../types';

export const QUESTIONS_DIR = 'questions';
/** Badge on question tiles and in the detail header. */
export const QUESTION_BADGE = '❓';

/** `questions/<name>` with a plain file name (no further folder). */
export function isQuestionPath(path: string): boolean {
  const prefix = `${QUESTIONS_DIR}/`;
  if (!path.startsWith(prefix)) return false;
  const name = path.slice(prefix.length);
  return name.length > 0 && !/[/\\]/.test(name);
}

export function isQuestion(tip: Pick<Tip, 'path'>): boolean {
  return isQuestionPath(tip.path);
}

/**
 * The kind to show. A question tip is never an agent task (the runner only
 * scans the top level), so `kind: agent` there shows as a plain note; the
 * file keeps its kind.
 */
export function displayKind(tip: Pick<Tip, 'path' | 'kind'>): Kind {
  return tip.kind === 'agent' && isQuestion(tip) ? 'note' : tip.kind;
}
