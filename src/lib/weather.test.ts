import { describe, expect, it } from 'vitest';
import { formatTemperature, weatherDescription, weatherLook } from './weather';

describe('weatherLook', () => {
  it('maps WMO codes to glyphs and text', () => {
    expect(weatherLook(0).glyph).toBe('☀');
    expect(weatherLook(0, false).glyph).toBe('☾');
    expect(weatherLook(3).en).toBe('Overcast');
    expect(weatherLook(63).zh).toBe('雨');
    expect(weatherLook(95).glyph).toBe('⚡');
    expect(weatherLook(123).en).toBe('Unknown');
    expect(weatherDescription(45, 'zh')).toBe('雾');
  });
});

describe('formatTemperature', () => {
  it('rounds to whole degrees', () => {
    expect(formatTemperature(23.4)).toBe('23°');
    expect(formatTemperature(-0.4)).toBe('0°');
    expect(formatTemperature(undefined)).toBe('–');
    expect(formatTemperature(Number.NaN)).toBe('–');
  });
});
