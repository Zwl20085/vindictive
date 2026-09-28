/**
 * WMO weather interpretation codes (as used by Open-Meteo) mapped to a
 * monochrome glyph and a short description in each UI language.
 */
import type { Language } from '../types';

export interface WeatherLook {
  glyph: string;
  en: string;
  zh: string;
}

const LOOKS: ReadonlyArray<[codes: readonly number[], look: WeatherLook]> = [
  [[0], { glyph: '☀', en: 'Clear', zh: '晴' }],
  [[1], { glyph: '☀', en: 'Mostly clear', zh: '大部晴朗' }],
  [[2], { glyph: '⛅', en: 'Partly cloudy', zh: '多云' }],
  [[3], { glyph: '☁', en: 'Overcast', zh: '阴' }],
  [[45, 48], { glyph: '≡', en: 'Fog', zh: '雾' }],
  [[51, 53, 55, 56, 57], { glyph: '☂', en: 'Drizzle', zh: '毛毛雨' }],
  [[61, 63, 65, 66, 67], { glyph: '☂', en: 'Rain', zh: '雨' }],
  [[71, 73, 75, 77], { glyph: '❄', en: 'Snow', zh: '雪' }],
  [[80, 81, 82], { glyph: '☂', en: 'Showers', zh: '阵雨' }],
  [[85, 86], { glyph: '❄', en: 'Snow showers', zh: '阵雪' }],
  [[95], { glyph: '⚡', en: 'Thunderstorm', zh: '雷暴' }],
  [[96, 99], { glyph: '⚡', en: 'Thunderstorm, hail', zh: '雷暴伴冰雹' }],
];

const NIGHT_CLEAR: WeatherLook = { glyph: '☾', en: 'Clear', zh: '晴' };
const UNKNOWN: WeatherLook = { glyph: '·', en: 'Unknown', zh: '未知' };

export function weatherLook(code: number, isDay = true): WeatherLook {
  if (!isDay && (code === 0 || code === 1)) return NIGHT_CLEAR;
  return LOOKS.find(([codes]) => codes.includes(code))?.[1] ?? UNKNOWN;
}

export function weatherDescription(code: number, lang: Language, isDay = true): string {
  return weatherLook(code, isDay)[lang];
}

/** `23°` with no decimals; Open-Meteo returns tenths. */
export function formatTemperature(celsius: number | undefined): string {
  if (celsius === undefined || Number.isNaN(celsius)) return '–';
  return `${Math.round(celsius)}°`;
}
