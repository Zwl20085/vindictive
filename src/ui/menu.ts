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
  document.body.append(menu);
  const rect = menu.getBoundingClientRect();
  const left = Math.max(0, Math.min(x, window.innerWidth - rect.width));
  const top = Math.max(0, Math.min(y, window.innerHeight - rect.height));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  openMenu = menu;
  document.addEventListener('pointerdown', onOutside, true);
  document.addEventListener('keydown', onKey, true);
  (menu.firstElementChild as HTMLElement | null)?.focus();
  return menu;
}
