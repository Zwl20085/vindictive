import { afterEach, describe, expect, it } from 'vitest';
import { formatDateTime, formatPanelDate, isLanguage, kindLabel, locale, setLocale, spanUnits, t, weekday } from './i18n';
import { countdown, formatLocal, formatSpan, DAY_MS, HOUR_MS } from './time';

afterEach(() => setLocale('en'));

describe('setLocale', () => {
  it('accepts only known languages and stamps the document', () => {
    setLocale('zh');
    expect(locale()).toBe('zh');
    expect(document.documentElement.lang).toBe('zh-CN');
    setLocale(undefined);
    expect(locale()).toBe('en');
    expect(document.documentElement.lang).toBe('en');
    expect(isLanguage('zh')).toBe(true);
    expect(isLanguage('fr')).toBe(false);
  });
});

describe('t', () => {
  it('translates and falls back to English', () => {
    expect(t('done')).toBe('Done');
    setLocale('zh');
    expect(t('done')).toBe('完成');
    expect(t('next', 'en')).toBe('NEXT');
    expect(kindLabel('deadline')).toBe('截止');
    expect(kindLabel('deadline', 'en')).toBe('Deadline');
  });
});

describe('dates', () => {
  const date = new Date(2026, 8, 28, 14, 3, 27); // Monday
  it('formats the panel date in both languages', () => {
    expect(formatPanelDate(date, 'en')).toBe('2026-09-28 MON');
    expect(formatPanelDate(date, 'zh')).toBe('2026年9月28日 周一');
    expect(weekday(date, 'zh')).toBe('周一');
  });
  it('formats detail dates in both languages', () => {
    expect(formatDateTime(date, 'en')).toBe('Sep 28, 14:03');
    expect(formatDateTime(date, 'zh')).toBe('9月28日 14:03');
    expect(formatLocal(date, 'zh')).toBe('9月28日 14:03');
  });
});

describe('Chinese countdowns', () => {
  const now = new Date(2026, 9, 1, 12, 0, 0);
  it('uses Chinese units without separators', () => {
    expect(spanUnits('zh').sep).toBe('');
    expect(formatSpan(2 * DAY_MS + 4 * HOUR_MS, 'zh')).toBe('2天4时');
    expect(formatSpan(5 * DAY_MS, 'zh')).toBe('5天');
    expect(formatSpan(45 * 60_000, 'zh')).toBe('45分');
    expect(countdown(new Date(now.getTime() - 2 * HOUR_MS), now, 'zh')).toBe('已逾期 2时');
    expect(countdown(new Date(now.getTime() + 3 * HOUR_MS), now, 'zh')).toBe('3时');
  });
});
