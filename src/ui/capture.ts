import { call, on } from '../api';
import { CAPTURE_HINT, CAPTURE_PLACEHOLDER, describeCapture } from '../lib/capture-hint';
import { el, mount } from './dom';

export const HINT_RESET_MS = 3000;

async function hide(): Promise<void> {
  try {
    await call('hide_capture');
  } catch (error) {
    console.error('Vindictive: hide_capture failed', error);
  }
}

/** Mount the quick-capture bar into `root`. */
export function renderCapture(root: HTMLElement): void {
  const input = el('input', {
    className: 'capture-input',
    type: 'text',
    placeholder: CAPTURE_PLACEHOLDER,
    autocomplete: 'off',
    spellcheck: 'false',
    'aria-label': 'New tip',
  });
  const hint = el('p', { className: 'capture-hint', text: CAPTURE_HINT, 'aria-live': 'polite' });
  let busy = false;

  const resetHint = (): void => {
    hint.textContent = CAPTURE_HINT;
    hint.dataset.state = '';
  };
  const setHint = (text: string, state: 'ok' | 'error'): void => {
    hint.textContent = text;
    hint.dataset.state = state;
    setTimeout(resetHint, HINT_RESET_MS);
  };

  const submit = async (): Promise<void> => {
    const text = input.value.trim();
    if (!text || busy) return;
    busy = true;
    input.disabled = true;
    try {
      await call('create_tip', { text });
      input.value = '';
      setHint('Saved', 'ok');
      await hide();
    } catch (error) {
      setHint(error instanceof Error ? error.message : String(error), 'error');
    } finally {
      busy = false;
      input.disabled = false;
      input.focus();
    }
  };

  input.addEventListener('input', () => {
    hint.textContent = input.value.trim() ? describeCapture(input.value) || CAPTURE_HINT : CAPTURE_HINT;
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') void submit();
    if (event.key === 'Escape') {
      input.value = '';
      resetHint();
      void hide();
    }
  });

  mount(root, el('div', { className: 'capture' }, input, hint));
  input.focus();

  void on('capture-shown', () => {
    input.value = '';
    resetHint();
    input.focus();
  }).catch((error) => console.error('Vindictive: cannot listen for capture-shown', error));
}
