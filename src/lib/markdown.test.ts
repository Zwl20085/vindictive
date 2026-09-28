import { describe, expect, it, vi } from 'vitest';
import { isRelativeSrc, renderMarkdown, resolveImages } from './markdown';

describe('renderMarkdown', () => {
  it('renders GFM and sanitises scripts', () => {
    const html = renderMarkdown('# Hi\n\n- [ ] item\n\n<script>alert(1)</script>\n\n**bold**');
    expect(html).toContain('<h1>Hi</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).not.toContain('<script');
  });
  it('makes links external', () => {
    const html = renderMarkdown('[x](https://example.org)');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });
  it('handles empty input', () => {
    expect(renderMarkdown('')).toBe('');
  });
});

describe('isRelativeSrc', () => {
  it('detects relative sources', () => {
    expect(isRelativeSrc('figures/a.png')).toBe(true);
    expect(isRelativeSrc('https://x/a.png')).toBe(false);
    expect(isRelativeSrc('data:image/png;base64,AA')).toBe(false);
    expect(isRelativeSrc(null)).toBe(false);
  });
});

describe('resolveImages', () => {
  it('rewrites relative images and leaves absolute ones', async () => {
    const root = document.createElement('div');
    root.innerHTML = renderMarkdown('![a](figures/a.png) ![b](https://x/b.png)');
    const resolver = vi.fn(async (src: string) => `data:${src}`);
    await resolveImages(root, resolver);
    const [a, b] = Array.from(root.querySelectorAll('img'));
    expect(a?.getAttribute('src')).toBe('data:figures/a.png');
    expect(a?.getAttribute('data-original-src')).toBe('figures/a.png');
    expect(b?.getAttribute('src')).toBe('https://x/b.png');
    expect(resolver).toHaveBeenCalledTimes(1);
  });
  it('marks failures without throwing', async () => {
    const root = document.createElement('div');
    root.innerHTML = '<img src="missing.png" alt="m">';
    const spy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    await resolveImages(root, async () => {
      throw new Error('404');
    });
    const img = root.querySelector('img');
    expect(img?.hasAttribute('src')).toBe(false);
    expect(img?.getAttribute('data-error')).toContain('404');
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});
