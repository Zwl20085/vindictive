/**
 * The status panel above the board: clock, date and weather. Owns one
 * element and updates it in place, so the clock can tick every second
 * without re-rendering the board.
 */
import type { Language, Weather } from '../types';
import { formatPanelDate, t } from '../lib/i18n';
import { formatClock } from '../lib/time';
import { formatTemperature, weatherLook } from '../lib/weather';
import { el } from './dom';

export type WeatherStatus = 'off' | 'loading' | 'ok' | 'error';

export interface PanelData {
  visible: boolean;
  language: Language;
  weather?: Weather;
  weatherStatus: WeatherStatus;
}

export const CLOCK_TICK_MS = 1000;

export class Panel {
  readonly element: HTMLElement;
  private readonly hm = el('span', { className: 'panel-hm' });
  private readonly sec = el('span', { className: 'panel-sec' });
  private readonly date = el('span', { className: 'panel-date' });
  private readonly glyph = el('span', { className: 'panel-glyph', 'aria-hidden': 'true' });
  private readonly temp = el('span', { className: 'panel-temp' });
  private readonly place = el('span', { className: 'panel-place' });
  private readonly detail = el('span', { className: 'panel-weather-detail' });
  private language: Language = 'en';
  private timer: ReturnType<typeof setInterval> | undefined;

  constructor() {
    this.element = el(
      'section',
      { className: 'panel', 'aria-label': 'clock and weather', 'data-tauri-drag-region': true },
      el('div', { className: 'panel-time' }, el('span', { className: 'panel-clock' }, this.hm, this.sec), this.date),
      el('div', { className: 'panel-weather' }, el('span', { className: 'panel-now' }, this.glyph, this.temp, this.place), this.detail),
    );
    this.tick();
  }

  /** Start the one-second clock. Idempotent. */
  start(): void {
    if (this.timer) return;
    this.timer = setInterval(() => this.tick(), CLOCK_TICK_MS);
  }

  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = undefined;
  }

  private tick(now = new Date()): void {
    this.hm.textContent = formatClock(now);
    this.sec.textContent = String(now.getSeconds()).padStart(2, '0');
    this.date.textContent = formatPanelDate(now, this.language);
  }

  update(data: PanelData): void {
    this.language = data.language;
    this.element.hidden = !data.visible;
    this.element.dataset.weather = data.weatherStatus;
    this.tick();
    const w = data.weather;
    if (data.weatherStatus === 'ok' && w) {
      const look = weatherLook(w.code, w.is_day);
      this.glyph.textContent = look.glyph;
      this.temp.textContent = formatTemperature(w.temperature_c);
      this.place.textContent = w.location;
      const range = w.high_c !== undefined && w.low_c !== undefined ? `${t('high')} ${formatTemperature(w.high_c)} ${t('low')} ${formatTemperature(w.low_c)} · ` : '';
      this.detail.textContent = `${range}${look[data.language]}`;
      this.element.title = `${w.location}: ${look[data.language]}, ${formatTemperature(w.temperature_c)}`;
      return;
    }
    this.glyph.textContent = data.weatherStatus === 'off' ? '' : '·';
    this.temp.textContent = '';
    this.place.textContent = '';
    this.detail.textContent =
      data.weatherStatus === 'loading' ? t('weatherLoading') : data.weatherStatus === 'error' ? t('weatherUnavailable') : t('weatherOff');
    this.element.title = this.detail.textContent;
  }
}
