/**
 * One-shot CSS animations driven from code. Every helper resolves early when
 * the user prefers reduced motion, so callers never wait on a skipped effect.
 */

export function reducedMotion(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
    : false;
}

/**
 * Add `className` to `node`, wait for its animation to finish (or `fallbackMs`
 * if no `animationend` arrives), then remove the class again.
 */
export function animate(node: HTMLElement, className: string, fallbackMs: number): Promise<void> {
  if (reducedMotion()) return Promise.resolve();
  return new Promise((resolve) => {
    let done = false;
    const finish = (): void => {
      if (done) return;
      done = true;
      node.removeEventListener('animationend', finish);
      node.classList.remove(className);
      resolve();
    };
    node.addEventListener('animationend', finish);
    node.classList.add(className);
    setTimeout(finish, fallbackMs + 50);
  });
}
