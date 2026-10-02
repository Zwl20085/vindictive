/**
 * Clawd's pixels, in units of the 16 x 11 sprite grid. Parts that only some
 * moods show are separate groups; `styles/panel.css` decides which are drawn.
 * A few parts (raised arms, the sweat drop) sit just outside the grid; the
 * SVG has `overflow: visible`.
 */

export type Rect = readonly [x: number, y: number, w: number, h: number];

export interface Part {
  className: string;
  rects: readonly Rect[];
}

export const TORSO: readonly Rect[] = [[2, 0, 12, 8]];

/** Two leg pairs that alternate while walking. */
export const LEGS_A: readonly Rect[] = [
  [3, 8, 1, 3],
  [10, 8, 1, 3],
];
export const LEGS_B: readonly Rect[] = [
  [5, 8, 1, 3],
  [12, 8, 1, 3],
];

/** Stubby arms at rest. */
const ARMS_REST: readonly Rect[] = [
  [0, 3, 2, 2],
  [14, 3, 2, 2],
];
/** Both arms thrown up: \o/ */
const ARMS_UP: readonly Rect[] = [
  [1, 2, 1, 2],
  [0, -1, 1, 3],
  [14, 2, 1, 2],
  [15, -1, 1, 3],
];

const EYES: readonly Rect[] = [
  [5, 2, 1, 2],
  [10, 2, 1, 2],
];
/** Squeezed-shut happy eyes: ^ ^ */
const EYES_HAPPY: readonly Rect[] = [
  [4, 3, 1, 1],
  [5, 2, 1, 1],
  [6, 3, 1, 1],
  [9, 3, 1, 1],
  [10, 2, 1, 1],
  [11, 3, 1, 1],
];
/** Eyes one row lower, leaving a gap under the brows. */
const EYES_WORRIED: readonly Rect[] = [
  [5, 3, 1, 2],
  [10, 3, 1, 2],
];
/** Brows raised at the inner ends, clear of the eyes and the head's edge. */
const BROWS: readonly Rect[] = [
  [3, 2, 1, 1],
  [4, 1, 1, 1],
  [11, 1, 1, 1],
  [12, 2, 1, 1],
];
const SMILE: readonly Rect[] = [
  [6, 5, 1, 1],
  [7, 6, 2, 1],
  [9, 5, 1, 1],
];
/** Wide open cheering mouth. */
const CHEER: readonly Rect[] = [
  [6, 5, 4, 1],
  [7, 6, 2, 1],
];
const FROWN: readonly Rect[] = [
  [6, 7, 1, 1],
  [7, 6, 2, 1],
  [9, 7, 1, 1],
];
const BLUSH: readonly Rect[] = [
  [3, 4, 2, 1],
  [11, 4, 2, 1],
];
/** A drop on the temple, outside the head. */
const SWEAT: readonly Rect[] = [
  [15, -2, 1, 1],
  [14, -1, 2, 2],
];

/** Everything above the legs, back to front. Sitting lowers this group. */
export const UPPER: readonly Part[] = [
  { className: 'clawd-body', rects: TORSO },
  { className: 'clawd-arms clawd-arms-rest', rects: ARMS_REST },
  { className: 'clawd-arms clawd-arms-up', rects: ARMS_UP },
  { className: 'clawd-eyes', rects: EYES },
  { className: 'clawd-face clawd-eyes-happy', rects: EYES_HAPPY },
  { className: 'clawd-face clawd-eyes-worried', rects: EYES_WORRIED },
  { className: 'clawd-face clawd-brows', rects: BROWS },
  { className: 'clawd-face clawd-smile', rects: SMILE },
  { className: 'clawd-face clawd-cheer', rects: CHEER },
  { className: 'clawd-face clawd-frown', rects: FROWN },
  { className: 'clawd-blush', rects: BLUSH },
  { className: 'clawd-sweat', rects: SWEAT },
];

/** Confetti flecks for a celebration: drift direction (px) and colour. */
export const CONFETTI: readonly { dx: number; dy: number; color: string }[] = [
  { dx: -22, dy: -10, color: '#f2c14e' },
  { dx: -14, dy: -16, color: '#6cc3a0' },
  { dx: -5, dy: -19, color: '#d97757' },
  { dx: 6, dy: -18, color: '#7fb2f0' },
  { dx: 15, dy: -15, color: '#f28ab2' },
  { dx: 22, dy: -9, color: '#f2c14e' },
];
