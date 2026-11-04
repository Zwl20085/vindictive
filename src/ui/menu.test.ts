import { afterEach, describe, expect, it, vi } from 'vitest';
import { closeMenu, showMenu, showSwatchMenu, type MenuItem } from './menu';

afterEach(() => closeMenu());

const options = (onPick: (c: string | null) => void, current?: string) => ({
  swatches: ['#4A3B6B', '#1F6F5C'],
  current,
  autoLabel: 'Auto',
  customLabel: 'Pick any colour',
  onPick,
});

describe('swatch menu', () => {
  it('picks a swatch and closes', () => {
    const onPick = vi.fn();
    const menu = showSwatchMenu(10, 10, options(onPick));
    const swatches = menu.querySelectorAll<HTMLButtonElement>('button.menu-swatch');
    expect(swatches).toHaveLength(2);
    swatches[1]?.click();
    expect(onPick).toHaveBeenCalledWith('#1F6F5C');
    expect(document.querySelector('.menu-swatches')).toBeNull();
  });

  it('Auto clears the override and marks the current colour', () => {
    const onPick = vi.fn();
    const menu = showSwatchMenu(10, 10, options(onPick, '#4a3b6b'));
    expect(menu.querySelector('button.menu-swatch')?.getAttribute('aria-pressed')).toBe('true');
    expect(menu.querySelector('.menu-swatch-auto')?.getAttribute('aria-pressed')).toBe('false');
    menu.querySelector<HTMLButtonElement>('.menu-swatch-auto')?.click();
    expect(onPick).toHaveBeenCalledWith(null);
  });

  it('the custom picker reports its colour', () => {
    const onPick = vi.fn();
    const menu = showSwatchMenu(10, 10, options(onPick));
    const input = menu.querySelector<HTMLInputElement>('input[type="color"]');
    if (!input) throw new Error('no picker');
    input.value = '#123456';
    input.dispatchEvent(new Event('change'));
    expect(onPick).toHaveBeenCalledWith('#123456');
  });
});

const key = (target: EventTarget, k: string): KeyboardEvent => {
  const event = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  return event;
};

function items(n: number, onSelect = vi.fn()): MenuItem[] {
  return Array.from({ length: n }, (_, i) => ({ label: `Item ${i}`, onSelect: () => onSelect(i) }));
}

describe('context menu keyboard', () => {
  it('Escape closes the menu without reaching bubble-phase handlers (e.g. closing the detail)', () => {
    const bubble = vi.fn();
    document.addEventListener('keydown', bubble);
    const opener = document.createElement('button');
    document.body.append(opener);
    opener.focus();
    showMenu(0, 0, items(2));
    key(document.activeElement ?? document.body, 'Escape');
    expect(document.querySelector('.menu')).toBeNull();
    expect(bubble).not.toHaveBeenCalled();
    document.removeEventListener('keydown', bubble);
    // With no menu open, Escape passes through as usual.
    key(document.body, 'Escape');
    opener.remove();
  });

  it('moves focus with ArrowDown/ArrowUp (wrapping), Home and End', () => {
    const menu = showMenu(0, 0, items(3));
    const li = Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]'));
    expect(document.activeElement).toBe(li[0]);
    expect(key(li[0]!, 'ArrowDown').defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(li[1]);
    key(li[1]!, 'ArrowDown');
    key(li[2]!, 'ArrowDown');
    expect(document.activeElement).toBe(li[0]);
    key(li[0]!, 'ArrowUp');
    expect(document.activeElement).toBe(li[2]);
    key(li[2]!, 'Home');
    expect(document.activeElement).toBe(li[0]);
    key(li[0]!, 'End');
    expect(document.activeElement).toBe(li[2]);
  });

  it('Enter and Space activate the item; Space does not scroll', () => {
    const onSelect = vi.fn();
    let menu = showMenu(0, 0, items(2, onSelect));
    const space = key(menu.querySelectorAll('[role="menuitem"]')[1]!, ' ');
    expect(space.defaultPrevented).toBe(true);
    expect(onSelect).toHaveBeenLastCalledWith(1);
    expect(document.querySelector('.menu')).toBeNull();
    menu = showMenu(0, 0, items(2, onSelect));
    key(menu.querySelector('[role="menuitem"]')!, 'Enter');
    expect(onSelect).toHaveBeenLastCalledWith(0);
  });

  it('returns focus to the element that had it at open', () => {
    const opener = document.createElement('button');
    document.body.append(opener);
    opener.focus();
    showMenu(0, 0, items(2));
    expect(document.activeElement).not.toBe(opener);
    key(document.activeElement!, 'Escape');
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it('returns focus to an explicit returnFocus element, also after a pick', () => {
    const target = document.createElement('button');
    document.body.append(target);
    const menu = showMenu(0, 0, items(1), target);
    menu.querySelector<HTMLElement>('[role="menuitem"]')?.click();
    expect(document.activeElement).toBe(target);
    target.remove();
  });
});
