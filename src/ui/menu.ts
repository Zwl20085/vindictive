import { el } from './dom';

export interface MenuItem {
  label: string;
  checked?: boolean;
  onSelect: () => void;
}

let openMenu: HTMLElement | undefined;
/** Where focus goes back to when the open menu closes. */
let returnTo: HTMLElement | undefined;

export function closeMenu(): void {
  const wasOpen = !!openMenu;
  openMenu?.remove();
  openMenu = undefined;
  document.removeEventListener('pointerdown', onOutside, true);
  document.removeEventListener('keydown', onKey, true);
  const target = returnTo;
  returnTo = undefined;
  if (wasOpen && target?.isConnected) target.focus();
}

function onOutside(event: Event): void {
  if (openMenu && !openMenu.contains(event.target as Node)) closeMenu();
}

function onKey(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return;
  // Swallow the key: a menu over the detail view must not also close the detail.
  event.stopPropagation();
  event.preventDefault();
  closeMenu();
}

export interface SwatchMenuOptions {
  /** Hex colours offered, in order. */
  swatches: readonly string[];
  /** Current override, or `undefined` for automatic. */
  current?: string;
  autoLabel: string;
  customLabel: string;
  /** `null` clears the override. */
  onPick: (color: string | null) => void;
  /** Focus returns here on close; defaults to whatever had focus at open. */
  returnFocus?: HTMLElement;
}

/** Put `menu` at `x, y`, clamped inside the viewport, and make it the open one. */
function open(menu: HTMLElement, x: number, y: number, returnFocus?: HTMLElement): HTMLElement {
  const active = document.activeElement;
  const opener = returnFocus ?? (active instanceof HTMLElement && active !== document.body ? active : undefined);
  document.body.append(menu);
  const rect = menu.getBoundingClientRect();
  const left = Math.max(0, Math.min(x, window.innerWidth - rect.width));
  const top = Math.max(0, Math.min(y, window.innerHeight - rect.height));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  openMenu = menu;
  returnTo = opener;
  document.addEventListener('pointerdown', onOutside, true);
  document.addEventListener('keydown', onKey, true);
  (menu.querySelector<HTMLElement>('[tabindex="0"], button, input') ?? null)?.focus();
  return menu;
}

/** A small colour grid at `x, y`: Auto, the swatches, and a custom picker. */
export function showSwatchMenu(x: number, y: number, options: SwatchMenuOptions): HTMLElement {
  closeMenu();
  const current = options.current?.toLowerCase();
  const pick = (color: string | null): void => {
    closeMenu();
    options.onPick(color);
  };
  const auto = el('button', { type: 'button', className: 'menu-swatch-auto', text: options.autoLabel, 'aria-pressed': String(!current) });
  auto.addEventListener('click', () => pick(null));
  const swatches = options.swatches.map((hex) => {
    const b = el('button', { type: 'button', className: 'swatch menu-swatch', title: hex, 'aria-label': hex, 'aria-pressed': String(current === hex.toLowerCase()) });
    b.style.setProperty('--swatch', hex);
    b.addEventListener('click', () => pick(hex));
    return b;
  });
  const custom = el('input', { type: 'color', className: 'swatch swatch-custom menu-swatch', title: options.customLabel, 'aria-label': options.customLabel, value: current ?? '#4a3b6b' });
  custom.addEventListener('change', () => pick(custom.value));
  const menu = el('div', { className: 'menu menu-swatches', role: 'dialog', 'aria-label': options.customLabel }, auto, el('div', { className: 'menu-swatch-grid' }, ...swatches, custom));
  return open(menu, x, y, options.returnFocus);
}

/** Arrow keys, Home and End move focus between the menu's items (wrapping). */
function roveFocus(menu: HTMLElement, event: KeyboardEvent): boolean {
  const items = Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]'));
  if (!items.length) return false;
  const at = items.indexOf(document.activeElement as HTMLElement);
  let next: number;
  switch (event.key) {
    case 'ArrowDown':
      next = at < 0 ? 0 : (at + 1) % items.length;
      break;
    case 'ArrowUp':
      next = at < 0 ? items.length - 1 : (at - 1 + items.length) % items.length;
      break;
    case 'Home':
      next = 0;
      break;
    case 'End':
      next = items.length - 1;
      break;
    default:
      return false;
  }
  event.preventDefault();
  items[next]?.focus();
  return true;
}

/**
 * Show a flat context menu at `x, y`, clamped inside the viewport. Focus
 * returns to `returnFocus` (or whatever had focus at open) when it closes.
 */
export function showMenu(x: number, y: number, items: MenuItem[], returnFocus?: HTMLElement): HTMLElement {
  closeMenu();
  const menu = el('ul', { className: 'menu', role: 'menu' });
  for (const item of items) {
    const li = el('li', { className: 'menu-item', role: 'menuitem', tabindex: '0' }, el('span', { className: 'menu-check', text: item.checked ? '✓' : '' }), item.label);
    const select = (): void => {
      closeMenu();
      item.onSelect();
    };
    li.addEventListener('click', select);
    li.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        select();
      }
    });
    menu.append(li);
  }
  menu.addEventListener('keydown', (e) => void roveFocus(menu, e));
  return open(menu, x, y, returnFocus);
}
