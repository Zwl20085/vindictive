import { el } from './dom';

export interface MenuItem {
  label: string;
  checked?: boolean;
  onSelect: () => void;
}

let openMenu: HTMLElement | undefined;

export function closeMenu(): void {
  openMenu?.remove();
  openMenu = undefined;
  document.removeEventListener('pointerdown', onOutside, true);
  document.removeEventListener('keydown', onKey, true);
}

function onOutside(event: Event): void {
  if (openMenu && !openMenu.contains(event.target as Node)) closeMenu();
}

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') closeMenu();
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
}

/** Put `menu` at `x, y`, clamped inside the viewport, and make it the open one. */
function open(menu: HTMLElement, x: number, y: number): HTMLElement {
  document.body.append(menu);
  const rect = menu.getBoundingClientRect();
  const left = Math.max(0, Math.min(x, window.innerWidth - rect.width));
  const top = Math.max(0, Math.min(y, window.innerHeight - rect.height));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  openMenu = menu;
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
  return open(menu, x, y);
}

/** Show a flat context menu at `x, y`, clamped inside the viewport. */
export function showMenu(x: number, y: number, items: MenuItem[]): HTMLElement {
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
      if (e.key === 'Enter' || e.key === ' ') select();
    });
    menu.append(li);
  }
  return open(menu, x, y);
}
