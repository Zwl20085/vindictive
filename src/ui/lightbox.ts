/** Full-window figure viewer. One at a time; Esc, click or Back closes it. */
import { t } from '../lib/i18n';
import { el } from './dom';

let open: HTMLElement | undefined;

export function closeLightbox(): void {
  if (!open) return;
  const node = open;
  open = undefined;
  document.removeEventListener('keydown', onKey, true);
  node.classList.add('lightbox-closing');
  setTimeout(() => node.remove(), 160);
}

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.stopPropagation();
    closeLightbox();
  }
}

export function isLightboxOpen(): boolean {
  return !!open;
}

export function showLightbox(src: string, caption: string): HTMLElement {
  closeLightbox();
  const img = el('img', { className: 'lightbox-img', src, alt: caption, draggable: 'false' });
  const close = el('button', { className: 'action lightbox-close', type: 'button', text: t('close') });
  const box = el(
    'div',
    { className: 'lightbox', role: 'dialog', 'aria-modal': 'true', 'aria-label': caption },
    img,
    el('div', { className: 'lightbox-bar' }, el('span', { className: 'lightbox-caption', text: caption }), close),
  );
  box.addEventListener('click', (event) => {
    if (event.target === box || event.target === img) closeLightbox();
  });
  close.addEventListener('click', closeLightbox);
  document.body.append(box);
  document.addEventListener('keydown', onKey, true);
  open = box;
  close.focus();
  return box;
}
