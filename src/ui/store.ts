/** Immutable UI state container with subscriptions. */
import type { BoardState, TipEdit, Weather } from '../types';
import type { WeatherStatus } from './panel';

/** Where an edit was opened from, and so where Save and Cancel return to. */
export type EditOrigin = 'board' | 'detail';

export type View =
  | { kind: 'board' }
  | { kind: 'detail'; id: string }
  | { kind: 'edit'; id: string; from?: EditOrigin }
  | { kind: 'settings' }
  /** The full form for a new tip, prefilled from `draft`. */
  | { kind: 'create'; draft: TipEdit };
export type SyncStatus = 'ok' | 'syncing' | 'error' | 'idle';

export interface UiState {
  board?: BoardState;
  view: View;
  sync: SyncStatus;
  weather?: Weather;
  weatherStatus: WeatherStatus;
  /** The inline "new tip" bar is open. */
  adding: boolean;
  /** The board shows only question tips (`questions/`). Not saved. */
  questionsOnly: boolean;
}

type Subscriber = (state: UiState, previous: UiState) => void;

export class Store {
  private state: UiState;
  private readonly subscribers = new Set<Subscriber>();

  constructor(initial: UiState = { view: { kind: 'board' }, sync: 'idle', weatherStatus: 'off', adding: false, questionsOnly: false }) {
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
