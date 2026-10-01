/**
 * Clawd, a small pixel-art companion who paces between the clock and the
 * weather. Pure SVG + CSS (see `styles/panel.css`); clicking him makes him
 * hop, and at night he curls up and sleeps.
 */

const SVG_NS = 'http://www.w3.org/2000/svg';
/** How long a hop (and its heart) lasts; matches `clawd-hop` in the CSS. */
export const HOP_MS = 700;
/** Clawd sleeps from this hour... */
export const SLEEP_FROM_HOUR = 23;
/** ...until this one. */
export const WAKE_AT_HOUR = 6;

type Rect = readonly [x: number, y: number, w: number, h: number];

/** Pixel grid, 16 x 11 units. */
const BODY: Rect[] = [
  [2, 0, 12, 8],
  [0, 3, 2, 2],
  [14, 3, 2, 2],
];
const EYES: Rect[] = [
  [5, 2, 1, 2],
  [10, 2, 1, 2],
];
/** Two leg pairs that alternate while walking. */
const LEGS_A: Rect[] = [
  [3, 8, 1, 3],
  [10, 8, 1, 3],
];
const LEGS_B: Rect[] = [
  [5, 8, 1, 3],
  [12, 8, 1, 3],
];

function group(className: string, rects: Rect[]): SVGGElement {
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

/** True during Clawd's night-time nap. */
export function isSleepTime(now: Date): boolean {
  const h = now.getHours();
  return h >= SLEEP_FROM_HOUR || h < WAKE_AT_HOUR;
}

export class Clawd {
  readonly element: HTMLElement;
  private readonly walker = span('clawd-walker');
  private hopTimer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('class', 'clawd');
    svg.setAttribute('viewBox', '0 0 16 11');
    svg.setAttribute('shape-rendering', 'crispEdges');
    const figure = document.createElementNS(SVG_NS, 'g');
    figure.setAttribute('class', 'clawd-figure');
    figure.append(group('clawd-legs clawd-legs-a', LEGS_A), group('clawd-legs clawd-legs-b', LEGS_B), group('clawd-body', BODY), group('clawd-eyes', EYES));
    svg.append(figure);

    const sprite = span('clawd-sprite');
    sprite.append(svg, span('clawd-heart', '♥'), span('clawd-z', 'z'));
    sprite.setAttribute('role', 'img');
    sprite.setAttribute('aria-label', 'Clawd');
    sprite.removeAttribute('aria-hidden');
    sprite.title = 'Clawd';
    sprite.addEventListener('click', () => this.hop());
    this.walker.append(sprite);
    this.element = span('clawd-track');
    this.element.append(this.walker);
  }

  /** A little jump with a heart. Clicking again restarts it. */
  hop(): void {
    this.element.classList.remove('clawd-hopping');
    // Force a reflow so the animation restarts on rapid clicks.
    void this.element.offsetWidth;
    this.element.classList.add('clawd-hopping');
    if (this.hopTimer) clearTimeout(this.hopTimer);
    this.hopTimer = setTimeout(() => this.element.classList.remove('clawd-hopping'), HOP_MS);
  }

  /** Called every clock tick; only touches the DOM when the state flips. */
  update(now: Date): void {
    const sleeping = isSleepTime(now);
    if ((this.element.dataset.sleeping === 'true') !== sleeping) this.element.dataset.sleeping = String(sleeping);
  }
}
