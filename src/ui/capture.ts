import { call, on } from '../api';
import { describeCapture } from '../lib/capture-hint';
import { setLocale, t } from '../lib/i18n';
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
    placeholder: t('capturePlaceholder'),
    autocomplete: 'off',
    spellcheck: 'false',
    'aria-label': 'New tip',
  });
  const hint = el('p', { className: 'capture-hint', text: t('captureHint'), 'aria-live': 'polite' });
  let busy = false;

  const applyLanguage = (): void => {
    input.placeholder = t('capturePlaceholder');
    if (!input.value.trim()) hint.textContent = t('captureHint');
  };
  const resetHint = (): void => {
    hint.textContent = t('captureHint');
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
      setHint(t('captureSaved'), 'ok');
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
    hint.textContent = input.value.trim() ? describeCapture(input.value) || t('captureHint') : t('captureHint');
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') void submit();
    if (event.key === 'Escape') {
      input.value = '';
      resetHint();
      void hide();
    }
  });

  mount(root, el('div', { className: 'capture' }, el('span', { className: 'capture-mark', 'aria-hidden': 'true' }), el('div', { className: 'capture-fields' }, input, hint)));
  input.focus();

  // Follow the board's language and theme.
  const applyState = (settings: { language?: string; theme?: string }): void => {
    setLocale(settings.language === 'zh' ? 'zh' : 'en');
    if (settings.theme) document.documentElement.dataset.theme = settings.theme;
    applyLanguage();
  };
  void call('get_state')
    .then((state) => applyState(state.settings))
    .catch(() => undefined);
  void on('board-updated', (state) => applyState(state.settings)).catch(() => undefined);

  void on('capture-shown', () => {
    input.value = '';
    resetHint();
    input.focus();
  }).catch((error) => console.error('Vindictive: cannot listen for capture-shown', error));
}
