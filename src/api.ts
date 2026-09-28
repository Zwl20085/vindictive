/**
 * Typed access to the Tauri backend. Falls back to an in-memory mock when
 * the page is opened in a plain browser (`npm run dev`), so the board can be
 * developed and screenshotted without the Rust side.
 */
import type { BoardState, Commands, Events } from './types';
import { mockInvoke, mockListen } from './mock/backend';

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

async function tauriListen<E extends keyof Events>(event: E, cb: Listener<Events[E]>): Promise<UnlistenFn> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<Events[E]>(event, (e) => cb(e.payload));
}

/** Call a backend command. Errors are rethrown as `Error` with context. */
export async function call<K extends CommandName>(cmd: K, args?: ArgsOf<K>): Promise<ResultOf<K>> {
  try {
    return isTauri() ? await tauriInvoke(cmd, args) : await mockInvoke(cmd, args);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Vindictive: command "${cmd}" failed`, args, error);
    throw new Error(`${cmd}: ${message}`);
  }
}

/** Subscribe to a backend event. Resolves to an unlisten function. */
export async function on<E extends keyof Events>(event: E, cb: Listener<Events[E]>): Promise<UnlistenFn> {
  return isTauri() ? tauriListen(event, cb) : mockListen(event, cb);
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
