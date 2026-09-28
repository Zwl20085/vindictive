/**
 * In-memory stand-in for the Rust backend. Used only when the page runs
 * outside Tauri. Every mutation produces new arrays; nothing is mutated.
 */
import type { BoardState, Commands, Events, Settings, Tip, Weather } from '../types';
import { MINUTE_MS, parseNaive, toNaive } from '../lib/time';
import { urgencyOf } from '../lib/urgency';
import { SAMPLE_SETTINGS, sampleTips } from './sample-tips';

type CommandName = keyof Commands;
type Listener = (payload: unknown) => void;

const URGENCY_POINTS = { overdue: 1000, critical: 500, soon: 200, later: 50, none: 10 } as const;
const PRIORITY_POINTS = { high: 30, normal: 10, low: 0 } as const;
const KIND_POINTS = { deadline: 5, event: 3, task: 2, reading: 1, note: 0 } as const;

/** A small SVG "plot" so figure rendering can be exercised without a repo. */
const PLACEHOLDER_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="240" height="140" viewBox="0 0 240 140">' +
  '<rect width="240" height="140" fill="#15141A"/>' +
  '<g stroke="#3A4356" stroke-width="1"><path d="M30 20V120H220"/><path d="M30 95H220M30 70H220M30 45H220" stroke-dasharray="2 4"/></g>' +
  '<polyline fill="none" stroke="#E0762B" stroke-width="2" points="30,110 60,96 90,88 120,70 150,62 180,40 210,30"/>' +
  '<polyline fill="none" stroke="#8ADF3C" stroke-width="1.5" stroke-dasharray="4 3" points="30,112 60,104 90,98 120,86 150,80 180,64 210,58"/>' +
  '<text x="36" y="132" font-family="Consolas, monospace" font-size="9" fill="#8A857D">f / kHz</text>' +
  '<text x="150" y="32" font-family="Consolas, monospace" font-size="9" fill="#E0762B">measured</text>' +
  '<text x="150" y="76" font-family="Consolas, monospace" font-size="9" fill="#8ADF3C">FE model</text>' +
  '</svg>';
const PLACEHOLDER_IMAGE = 'data:image/svg+xml;utf8,' + encodeURIComponent(PLACEHOLDER_SVG);

const WEATHER_DELAY_MS = 600;

interface MockStore {
  tips: Tip[];
  settings: Settings;
  token: string | undefined;
  lastSync: string | undefined;
}

let store: MockStore = {
  tips: sampleTips(new Date()),
  settings: SAMPLE_SETTINGS,
  token: 'mock-token',
  lastSync: toNaive(new Date()),
};

const listeners = new Map<string, Set<Listener>>();

function emit<E extends keyof Events>(event: E, payload: Events[E]): void {
  listeners.get(event)?.forEach((cb) => cb(payload));
}

export function mockListen<E extends keyof Events>(event: E, cb: (payload: Events[E]) => void): () => void {
  const set = listeners.get(event) ?? new Set<Listener>();
  set.add(cb as Listener);
  listeners.set(event, set);
  return () => set.delete(cb as Listener);
}

function score(tip: Tip, now: Date): number {
  const urgency = urgencyOf(parseNaive(tip.due_at), now);
  return URGENCY_POINTS[urgency] + PRIORITY_POINTS[tip.priority] + KIND_POINTS[tip.kind];
}

function isVisible(tip: Tip, now: Date): boolean {
  const snoozed = parseNaive(tip.snoozed_until_at);
  return tip.status === 'open' && !(snoozed && snoozed > now);
}

function buildState(): BoardState {
  const now = new Date();
  const tips = [...store.tips].sort((a, b) => score(b, now) - score(a, now) || a.title.localeCompare(b.title));
  const next = tips.find((t) => isVisible(t, now));
  return {
    tips,
    next_up: next?.id,
    last_sync: store.lastSync,
    has_token: !!store.token,
    settings: store.settings,
    now: toNaive(now),
    pending_edits: 0,
  };
}

function update(patch: Partial<MockStore>): BoardState {
  store = { ...store, ...patch };
  const state = buildState();
  emit('board-updated', state);
  return state;
}

function replaceTip(id: string, fn: (tip: Tip) => Tip): Tip[] {
  const found = store.tips.some((t) => t.id === id);
  if (!found) throw new Error(`no tip with id ${id}`);
  return store.tips.map((t) => (t.id === id ? fn(t) : t));
}

function slug(text: string): string {
  return (
    text
      .toLowerCase()
      .replace(/[^a-z0-9一-鿿]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 48) || 'tip'
  );
}

function createTip(text: string): Tip {
  const title = text
    .split(/\s+/)
    .filter((w) => !/^[#!@^>＃！＠＾＞]/.test(w))
    .join(' ')
    .trim();
  if (!title) throw new Error('capture text has no title');
  const id = `${toNaive(new Date()).slice(0, 10)}-${slug(title)}`;
  const priority = /!high\b/.test(text) ? 'high' : /!low\b/.test(text) ? 'low' : 'normal';
  const tags = Array.from(text.matchAll(/[#＃]([\w一-鿿]+)/g)).map((m) => m[1] ?? '');
  return { id, path: `tips/${id}.md`, title, kind: 'task', priority, status: 'open', tags, body: '', remind_at: [] };
}

function mockWeather(location: string): Weather {
  const hour = new Date().getHours();
  return {
    location,
    temperature_c: 23.4,
    high_c: 26.1,
    low_c: 15.7,
    humidity: 58,
    wind_kmh: 14,
    code: 2,
    is_day: hour >= 6 && hour < 18,
    fetched_at: toNaive(new Date()),
  };
}

const handlers: { [K in CommandName]: (args: Parameters<Commands[K]>[0]) => ReturnType<Commands[K]> | Promise<ReturnType<Commands[K]>> } = {
  get_state: () => buildState(),
  sync_now: () => update({ lastSync: toNaive(new Date()) }),
  mark_done: ({ id }) =>
    update({ tips: replaceTip(id, (t) => ({ ...t, status: 'done', done_at: toNaive(new Date()) })) }),
  reopen: ({ id }) => update({ tips: replaceTip(id, (t) => ({ ...t, status: 'open', done_at: undefined })) }),
  snooze: ({ id, minutes }) => {
    const until = toNaive(new Date(Date.now() + minutes * MINUTE_MS));
    return update({ tips: replaceTip(id, (t) => ({ ...t, snoozed_until: until, snoozed_until_at: until })) });
  },
  create_tip: ({ text }) => update({ tips: [createTip(text), ...store.tips] }),
  push_now: () => buildState(),
  fit_window: () => undefined,
  set_color: ({ id, color }) => update({ tips: replaceTip(id, (t) => ({ ...t, color: color ?? undefined })) }),
  set_order: ({ id, order }) => update({ tips: replaceTip(id, (t) => ({ ...t, order: order ?? undefined })) }),
  set_size: ({ id, size }) => update({ tips: replaceTip(id, (t) => ({ ...t, size: size ?? undefined })) }),
  edit_local: ({ id }) => {
    const tip = store.tips.find((t) => t.id === id);
    if (!tip) throw new Error(`no tip with id ${id}`);
    return `C:\\Users\\you\\AppData\\Roaming\\dev.zhangwentao.vindictive\\edit\\${tip.path.replace(/\//g, '\\')}`;
  },
  delete_tip: ({ id }) => {
    if (!store.tips.some((t) => t.id === id)) throw new Error(`no tip with id ${id}`);
    return update({ tips: store.tips.filter((t) => t.id !== id) });
  },
  frontend_log: ({ level, message }) => {
    if (level === 'error') console.error(`[mock log] ${message}`);
  },
  save_settings: ({ settings }) => update({ settings: { ...settings } }),
  set_token: ({ token }) => {
    if (!token.trim()) throw new Error('token is empty');
    store = { ...store, token };
  },
  clear_token: () => {
    store = { ...store, token: undefined };
  },
  test_connection: () => {
    if (!store.token) throw new Error('no token stored');
    return `OK: ${store.tips.length} tips in ${store.settings.owner}/${store.settings.repo}/${store.settings.dir}`;
  },
  fetch_image: ({ path }) => (/^(https?:|data:)/i.test(path) ? path : PLACEHOLDER_IMAGE),
  enrich_tip: ({ id }) => update({ tips: replaceTip(id, (t) => t) }),
  fetch_weather: async () => {
    const place = store.settings.weather_location.trim();
    if (!place) return null;
    await new Promise((r) => setTimeout(r, WEATHER_DELAY_MS));
    if (/^(nowhere|error)$/i.test(place)) throw new Error(`no such place: ${place}`);
    return mockWeather(place.charAt(0).toUpperCase() + place.slice(1));
  },
  show_capture: () => undefined,
  hide_capture: () => undefined,
  dock_window: () => undefined,
  quit: () => undefined,
};

export async function mockInvoke<K extends CommandName>(
  cmd: K,
  args?: Parameters<Commands[K]>[0],
): Promise<ReturnType<Commands[K]>> {
  const handler = handlers[cmd] as (a: unknown) => ReturnType<Commands[K]> | Promise<ReturnType<Commands[K]>>;
  return handler(args);
}
