/** Immutable UI state container with subscriptions. */
import type { BoardState } from '../types';

export type View = { kind: 'board' } | { kind: 'detail'; id: string } | { kind: 'settings' };
export type SyncStatus = 'ok' | 'syncing' | 'error' | 'idle';

export interface UiState {
  board?: BoardState;
  view: View;
  sync: SyncStatus;
}

type Subscriber = (state: UiState, previous: UiState) => void;

export class Store {
  private state: UiState;
  private readonly subscribers = new Set<Subscriber>();

  constructor(initial: UiState = { view: { kind: 'board' }, sync: 'idle' }) {
    this.state = initial;
  }

  get(): UiState {
    return this.state;
  }

  /** Replace state with a new object; never mutates the old one. */
  set(patch: Partial<UiState>): void {
    const previous = this.state;
    this.state = { ...previous, ...patch };
    this.subscribers.forEach((cb) => cb(this.state, previous));
  }

  subscribe(cb: Subscriber): () => void {
    this.subscribers.add(cb);
    return () => this.subscribers.delete(cb);
  }
}
