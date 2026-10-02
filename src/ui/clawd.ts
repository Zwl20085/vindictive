/**
 * Clawd, a small pixel-art companion who paces between the clock and the
 * weather and reacts to the board: he cheers when a tip is completed, frets
 * while something is overdue, sits down contentedly when nothing is left
 * and sleeps at night. Pure SVG + CSS (see `styles/panel.css`); the mood
 * itself is decided in `lib/mood.ts`.
 */
import type { Tip } from '../types';
import { clawdFeeling, clawdMood, moodCounts, shouldCelebrate, type ClawdMood, type MoodCounts } from '../lib/mood';
import { CONFETTI, LEGS_A, LEGS_B, UPPER, type Rect } from './clawd-pixels';

export { isSleepTime, SLEEP_FROM_HOUR, WAKE_AT_HOUR } from '../lib/mood';

const SVG_NS = 'http://www.w3.org/2000/svg';
/** How long a hop (and its heart) lasts; matches `clawd-hop` in the CSS. */
export const HOP_MS = 700;
/** How long the party lasts after a tip is completed. */
export const CELEBRATE_MS = 2500;
/** From this many overdue tips on, Clawd frets a little harder. */
export const WORRY_HARDER_FROM = 3;

function group(className: string, rects: readonly Rect[]): SVGGElement {
  const g = document.createElementNS(SVG_NS, 'g');
  g.setAttribute('class', className);
  for (const [x, y, w, h] of rects) {
    const r = document.createElementNS(SVG_NS, 'rect');
    r.setAttribute('x', String(x));
    r.setAttribute('y', String(y));
    r.setAttribute('width', String(w));
    r.setAttribute('height', String(h));
    g.append(r);
  }
  return g;
}

function span(className: string, text = ''): HTMLSpanElement {
  const s = document.createElement('span');
  s.className = className;
  s.textContent = text;
  s.setAttribute('aria-hidden', 'true');
  return s;
}

function drawing(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('class', 'clawd');
  svg.setAttribute('viewBox', '0 0 16 11');
  svg.setAttribute('shape-rendering', 'crispEdges');
  const upper = document.createElementNS(SVG_NS, 'g');
  upper.setAttribute('class', 'clawd-upper');
  upper.append(...UPPER.map((part) => group(part.className, part.rects)));
  const figure = document.createElementNS(SVG_NS, 'g');
  figure.setAttribute('class', 'clawd-figure');
  figure.append(group('clawd-legs clawd-legs-a', LEGS_A), group('clawd-legs clawd-legs-b', LEGS_B), upper);
  svg.append(figure);
  return svg;
}

function confetti(): HTMLSpanElement[] {
  return CONFETTI.map(({ dx, dy, color }, i) => {
    const fleck = span('clawd-confetti');
    fleck.style.setProperty('--dx', `${dx}px`);
    fleck.style.setProperty('--dy', `${dy}px`);
    fleck.style.setProperty('--c', color);
    fleck.style.setProperty('--i', String(i));
    return fleck;
  });
}

function worryLevel(mood: ClawdMood, counts: MoodCounts): string {
  if (mood !== 'worried') return '0';
  return counts.overdue >= WORRY_HARDER_FROM ? '2' : '1';
}

export class Clawd {
  readonly element: HTMLElement;
  private readonly walker = span('clawd-walker');
  private readonly sprite = span('clawd-sprite');
  private hopTimer: ReturnType<typeof setTimeout> | undefined;
  private partyTimer: ReturnType<typeof setTimeout> | undefined;
  private tips: readonly Tip[] | undefined;
  private now = new Date();
  private forced: ClawdMood | undefined;

  constructor() {
    this.sprite.append(drawing(), span('clawd-heart', '♥'), span('clawd-z', 'z'), span('clawd-note', '♪'), ...confetti());
    this.sprite.setAttribute('role', 'img');
    this.sprite.setAttribute('aria-label', 'Clawd');
    this.sprite.removeAttribute('aria-hidden');
    this.sprite.title = 'Clawd';
    this.sprite.addEventListener('click', () => this.hop());
    this.walker.append(this.sprite);
    this.element = span('clawd-track');
    this.element.append(this.walker);
    this.refresh();
  }

  /** A little jump with a heart. Clicking again restarts it; ignored mid-party. */
  hop(): void {
    if (this.element.dataset.mood === 'celebrating') return;
    this.element.classList.remove('clawd-hopping');
    // Force a reflow so the animation restarts on rapid clicks.
    void this.element.offsetWidth;
    this.element.classList.add('clawd-hopping');
    if (this.hopTimer) clearTimeout(this.hopTimer);
    this.hopTimer = setTimeout(() => this.element.classList.remove('clawd-hopping'), HOP_MS);
  }

  /** Jump for joy for `CELEBRATE_MS`; a new completion restarts the party. */
  celebrate(): void {
    if (this.partyTimer) clearTimeout(this.partyTimer);
    this.element.classList.remove('clawd-hopping');
    this.partyTimer = setTimeout(() => {
      this.partyTimer = undefined;
      this.refresh();
    }, CELEBRATE_MS);
    this.refresh();
  }

  /** A new board arrived: celebrate fresh completions, then re-read the mood. */
  setTips(tips: readonly Tip[] | undefined, now: Date): void {
    if (tips === this.tips) return this.update(now);
    const previous = this.tips;
    this.tips = tips;
    this.now = now;
    if (tips && shouldCelebrate(previous, tips)) this.celebrate();
    else this.refresh();
  }

  /** Called every clock tick; only touches the DOM when something changed. */
  update(now: Date): void {
    this.now = now;
    this.refresh();
  }

  /** Hold one mood regardless of the board (dev screenshots); `undefined` releases it. */
  force(mood: ClawdMood | undefined): void {
    this.forced = mood;
    this.refresh();
  }

  private refresh(): void {
    const counts = moodCounts(this.tips ?? [], this.now);
    const mood = this.forced ?? clawdMood({ tips: this.tips, now: this.now, celebrating: !!this.partyTimer });
    const data = this.element.dataset;
    if (data.mood !== mood) data.mood = mood;
    const level = worryLevel(mood, counts);
    if (data.level !== level) data.level = level;
    const feeling = clawdFeeling(mood, counts);
    if (this.sprite.title !== feeling) this.sprite.title = feeling;
  }
}
