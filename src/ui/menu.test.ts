import { afterEach, describe, expect, it, vi } from 'vitest';
import { closeMenu, showSwatchMenu } from './menu';

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
