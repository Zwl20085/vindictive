import { call } from '../api';
import { el } from './dom';

export const ERROR_VISIBLE_MS = 5000;

let line: HTMLElement | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;

/** The single red line that lives under the chrome strip. */
export function errorLine(): HTMLElement {
  if (!line) line = el('div', { className: 'error-line', role: 'alert', 'aria-live': 'assertive' });
  return line;
}

/** A neutral, non-error message on the same line (e.g. "Opened <path>"). */
export function showNotice(context: string, message: string): void {
  void call('frontend_log', { level: 'info', message: `${context}: ${message}` }).catch(() => undefined);
  const node = errorLine();
  node.textContent = `${context}: ${message}`;
  node.classList.add('visible', 'notice');
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => node.classList.remove('visible', 'notice'), ERROR_VISIBLE_MS);
}

export function showError(context: string, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Vindictive: ${context}`, error);
  void call('frontend_log', { level: 'error', message: `${context}: ${message}` }).catch(() => undefined);
  const node = errorLine();
  node.textContent = `${context}: ${message}`;
  node.classList.remove('notice');
  node.classList.add('visible');
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => node.classList.remove('visible'), ERROR_VISIBLE_MS);
}

/** Run an async action, routing any failure to the error line. */
export async function guard<T>(context: string, action: () => Promise<T>): Promise<T | undefined> {
  try {
    return await action();
  } catch (error) {
    showError(context, error);
    return undefined;
  }
}
