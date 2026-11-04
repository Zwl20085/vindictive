/**
 * Typed access to the Tauri backend. Falls back to an in-memory mock when
 * the page is opened in a plain browser (`npm run dev`), so the board can be
 * developed and screenshotted without the Rust side.
 */
import type { BoardState, Commands, Events } from './types';
import { errorMessage } from './ui/errors';

type CommandName = keyof Commands;
type ArgsOf<K extends CommandName> = Parameters<Commands[K]>[0];
type ResultOf<K extends CommandName> = ReturnType<Commands[K]>;

type UnlistenFn = () => void;
type Listener<T> = (payload: T) => void;

const TAURI_MARKER = '__TAURI_INTERNALS__';

export function isTauri(): boolean {
  return typeof window !== 'undefined' && TAURI_MARKER in window;
}

async function tauriInvoke<K extends CommandName>(cmd: K, args?: ArgsOf<K>): Promise<ResultOf<K>> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<ResultOf<K>>(cmd, args as Record<string, unknown> | undefined);
}

/**
 * The browser mock, loaded only outside Tauri so it (and its sample tips)
 * stays out of the code that runs in the app.
 */
function mock(): Promise<typeof import('./mock/backend')> {
  return import('./mock/backend');
}

async function tauriListen<E extends keyof Events>(event: E, cb: Listener<Events[E]>): Promise<UnlistenFn> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<Events[E]>(event, (e) => cb(e.payload));
}

/** Call a backend command. Errors are rethrown as `Error` with context. */
export async function call<K extends CommandName>(cmd: K, args?: ArgsOf<K>): Promise<ResultOf<K>> {
  try {
    return isTauri() ? await tauriInvoke(cmd, args) : await (await mock()).mockInvoke(cmd, args);
  } catch (error) {
    const message = errorMessage(error);
    console.error(`Vindictive: command "${cmd}" failed`, args, error);
    throw new Error(`${cmd}: ${message}`);
  }
}

/** Subscribe to a backend event. Resolves to an unlisten function. */
export async function on<E extends keyof Events>(event: E, cb: Listener<Events[E]>): Promise<UnlistenFn> {
  if (isTauri()) return tauriListen(event, cb);
  return (await mock()).mockListen(event, cb);
}

export function onBoardUpdated(cb: Listener<BoardState>): Promise<UnlistenFn> {
  return on('board-updated', cb);
}

/** Open a URL in the system browser. */
export async function openExternal(url: string): Promise<void> {
  if (!/^https?:\/\//i.test(url)) throw new Error(`refusing to open non-http url: ${url}`);
  if (isTauri()) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } else {
    window.open(url, '_blank', 'noopener');
  }
}
