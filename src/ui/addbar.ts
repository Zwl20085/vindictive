/**
 * Inline "new tip" bar shown at the top of the board. Same syntax as the
 * quick-capture window, but inside the main window so it works with the
 * mouse and in the browser mock.
 */
import { describeCapture } from '../lib/capture-hint';
import { t } from '../lib/i18n';
import { el } from './dom';

export interface AddBarActions {
  onSubmit: (text: string) => Promise<void>;
  onCancel: () => void;
}

export function renderAddBar(actions: AddBarActions): HTMLElement {
  const input = el('input', {
    className: 'addbar-input',
    type: 'text',
    placeholder: t('capturePlaceholder'),
    autocomplete: 'off',
    spellcheck: 'false',
    'aria-label': t('addTip'),
  });
  const hint = el('p', { className: 'addbar-hint', text: t('captureHint') });
  const cancel = el('button', { className: 'action action-back', type: 'button', text: t('cancel') });
  let busy = false;

  const submit = async (): Promise<void> => {
    const text = input.value.trim();
    if (!text || busy) return;
    busy = true;
    input.disabled = true;
    try {
      await actions.onSubmit(text);
    } catch (error) {
      hint.textContent = error instanceof Error ? error.message : String(error);
      hint.dataset.state = 'error';
    } finally {
      busy = false;
      input.disabled = false;
      input.focus();
    }
  };

  input.addEventListener('input', () => {
    hint.dataset.state = '';
    hint.textContent = input.value.trim() ? describeCapture(input.value) || t('captureHint') : t('captureHint');
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') void submit();
    if (event.key === 'Escape') {
      event.stopPropagation();
      actions.onCancel();
    }
  });
  cancel.addEventListener('click', actions.onCancel);

  const bar = el('div', { className: 'addbar', role: 'form', 'aria-label': t('addTip') }, el('div', { className: 'addbar-row' }, input, cancel), hint);
  // Focus once the bar is in the document.
  requestAnimationFrame(() => input.focus());
  return bar;
}
