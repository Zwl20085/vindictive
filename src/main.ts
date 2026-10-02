import './styles/tokens.css';
import './styles/chrome.css';
import './styles/panel.css';
import './styles/clawd.css';
import './styles/board.css';
import './styles/tile.css';
import './styles/detail.css';
import './styles/settings.css';
import { on } from './api';
import { App } from './ui/app';
import { showError } from './ui/errors';

function boot(): void {
  const root = document.getElementById('app');
  if (!root) throw new Error('#app root element missing');
  const app = new App(root);
  app.start().catch((error) => showError('startup', error));
  on('open-settings', () => app.openSettings()).catch((error) => showError('event subscription', error));
}

boot();
