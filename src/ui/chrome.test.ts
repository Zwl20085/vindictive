import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderChrome, type ChromeActions } from './chrome';
import { closeMenu } from './menu';

afterEach(() => {
  closeMenu();
  document.body.replaceChildren();
});

function actions(): ChromeActions {
  return { onSync: vi.fn(), onOpenFolder: vi.fn(), onSettings: vi.fn(), onToggleDone: vi.fn(), onToggleQuestions: vi.fn(), onDock: vi.fn(), onQuit: vi.fn() };
}

describe('chrome strip', () => {
  it('draws the sync square as a focusable button with a live status next to it', () => {
    const strip = renderChrome({ sync: 'syncing', actions: actions() });
    const dot = strip.querySelector('.sync-dot');
    expect(dot?.tagName).toBe('BUTTON');
    expect(dot?.getAttribute('aria-haspopup')).toBe('menu');
    expect(dot?.getAttribute('aria-label')).toMatch(/Syncing/);
    expect(strip.querySelector('[role="status"]')?.textContent).toBe('Syncing…');
  });

  it('a click on the square (also what Enter and Space do on a button) syncs and opens no menu', () => {
    const a = actions();
    const strip = renderChrome({ sync: 'idle', actions: a });
    document.body.append(strip);
    strip.querySelector<HTMLButtonElement>('.sync-dot')?.click();
    expect(a.onSync).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.menu')).toBeNull();
  });

  it('the menu key and Shift+F10 on the square open the strip menu', () => {
    const a = actions();
    const strip = renderChrome({ sync: 'idle', actions: a });
    document.body.append(strip);
    const dot = strip.querySelector<HTMLButtonElement>('.sync-dot');
    if (!dot) throw new Error('no dot');
    dot.focus();
    const menuKey = new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true });
    dot.dispatchEvent(menuKey);
    expect(menuKey.defaultPrevented).toBe(true);
    const menu = document.querySelector('.menu');
    expect(menu).not.toBeNull();
    // First item reloads the folder; focus comes back to the square.
    menu?.querySelector<HTMLElement>('[role="menuitem"]')?.click();
    expect(a.onSync).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(dot);
    dot.dispatchEvent(new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true, cancelable: true }));
    expect(document.querySelector('.menu')).not.toBeNull();
    closeMenu();
    // Plain F10 does nothing.
    dot.dispatchEvent(new KeyboardEvent('keydown', { key: 'F10', bubbles: true, cancelable: true }));
    expect(document.querySelector('.menu')).toBeNull();
  });

  it('Escape closes the menu and puts focus back on the square', () => {
    const strip = renderChrome({ sync: 'idle', actions: actions() });
    document.body.append(strip);
    const dot = strip.querySelector<HTMLButtonElement>('.sync-dot');
    dot?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ContextMenu', bubbles: true, cancelable: true }));
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(document.querySelector('.menu')).toBeNull();
    expect(document.activeElement).toBe(dot);
  });

  it('still opens the menu on right-click', () => {
    const strip = renderChrome({ sync: 'idle', actions: actions() });
    document.body.append(strip);
    strip.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 30, clientY: 5 }));
    expect(document.querySelector('.menu')).not.toBeNull();
  });

  it('the menu has a "Questions only" filter that shows its state', () => {
    const a = actions();
    for (const on of [false, true]) {
      closeMenu();
      const strip = renderChrome({ sync: 'idle', actions: a, questionsOnly: on });
      document.body.append(strip);
      strip.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 30, clientY: 5 }));
      const item = Array.from(document.querySelectorAll<HTMLElement>('.menu [role="menuitem"]')).find((i) => i.textContent?.includes('Questions only'));
      expect(item, 'menu item').toBeDefined();
      expect(item?.querySelector('.menu-check')?.textContent).toBe(on ? '✓' : '');
      item?.click();
    }
    expect(a.onToggleQuestions).toHaveBeenCalledTimes(2);
  });
});
