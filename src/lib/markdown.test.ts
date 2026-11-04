import { describe, expect, it, vi } from 'vitest';
import { isRelativeSrc, isSafeAssetUrl, isSvgSource, looksLikeImage, renderMarkdown, resolveImages } from './markdown';

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
  it('keeps inline svg but strips scripts and handlers inside it', () => {
    const html = renderMarkdown(
      '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" onload="alert(1)"><circle cx="5" cy="5" r="4" fill="#E0762B"/><script>alert(2)</script><foreignObject><div>x</div></foreignObject></svg>',
    );
    expect(html).toContain('<svg');
    expect(html).toContain('<circle');
    expect(html).not.toContain('onload');
    expect(html).not.toContain('<script');
    expect(html).not.toContain('foreignObject');
  });
  it('drops javascript: links but keeps http ones', () => {
    const html = renderMarkdown('[bad](javascript:alert(1)) <a href="JaVaScRiPt:alert(2)">x</a> [ok](https://example.org/a)');
    expect(html.toLowerCase()).not.toContain('javascript:');
    expect(html).toContain('href="https://example.org/a"');
  });
  it('drops onload on a bare svg and handlers on svg children', () => {
    const html = renderMarkdown('<svg onload="alert(1)"><rect onclick="alert(2)" onmouseover="x()" width="1" height="1"/></svg>');
    expect(html).toContain('<rect');
    expect(html).not.toMatch(/on(load|click|mouseover)/);
  });
  it('drops style attributes and data- attributes', () => {
    const html = renderMarkdown('<p style="position:fixed;inset:0" data-x="1">hi</p><svg><circle style="fill:red" r="1"/></svg>');
    expect(html).toContain('<p>hi</p>');
    expect(html).not.toContain('style=');
    expect(html).not.toContain('data-x');
  });
  it('drops http, protocol-relative and UNC image sources and srcset; keeps https, relative and data ones', () => {
    const html = renderMarkdown(
      '![r](http://plain.example/p.gif) <img src="//proto.example/q.gif"> <img src="\\\\unc.example\\share\\u.png"> ' +
        '<img src="https://ok.example/a.png" srcset="https://ok.example/b.png 2x"> ' +
        '![l](figures/plot.png) <img src="data:image/png;base64,AAAA">' +
        '<svg><image href="http://plain.example/s.png"/><image xlink:href="//proto.example/t.png"/><image href="figs/ok.png"/>' +
        '<filter id="f"><feImage href="http://plain.example/f.png"/></filter></svg>',
    );
    expect(html).not.toContain('plain.example');
    expect(html).not.toContain('proto.example');
    expect(html).not.toContain('unc.example');
    expect(html).not.toContain('srcset');
    expect(html).toContain('src="https://ok.example/a.png"');
    expect(html).toContain('src="figures/plot.png"');
    expect(html).toContain('src="data:image/png;base64,AAAA"');
    expect(html).toContain('figs/ok.png');
  });
  it('keeps svg gradients, markers and in-page anchors pointing at the prefixed ids', () => {
    const html = renderMarkdown(
      '<svg><defs><linearGradient id="g"><stop offset="0"/></linearGradient><marker id="m"/></defs>' +
        `<rect fill="url(#g)" marker-end="url( '#m')" stroke="url(#user-content-g)"/></svg>` +
        '\n\n[x](#sec)',
    );
    expect(html).toContain('id="user-content-g"');
    expect(html).toContain('fill="url(#user-content-g)"');
    expect(html).toContain(`marker-end="url('#user-content-m')"`);
    expect(html).toContain('stroke="url(#user-content-g)"');
    expect(html).toContain('href="#user-content-sec"');
  });
  it('names cannot clobber document properties', () => {
    const html = renderMarkdown('<img name="getElementById" id="cookie" src="a.png">');
    expect(html).not.toMatch(/name="getElementById"|id="cookie"/);
  });
});

describe('source helpers', () => {
  it('detects relative sources', () => {
    expect(isRelativeSrc('figures/a.png')).toBe(true);
    expect(isRelativeSrc('https://x/a.png')).toBe(false);
    expect(isRelativeSrc('data:image/png;base64,AA')).toBe(false);
    expect(isRelativeSrc(null)).toBe(false);
  });
  it('allows only relative, fragment, data: and https: asset URLs', () => {
    expect(isSafeAssetUrl('figures/a.png')).toBe(true);
    expect(isSafeAssetUrl('../a.png')).toBe(true);
    expect(isSafeAssetUrl('#grad')).toBe(true);
    expect(isSafeAssetUrl(' data:image/png;base64,AA')).toBe(true);
    expect(isSafeAssetUrl('https://x/a.png')).toBe(true);
    expect(isSafeAssetUrl('HTTPS://x/a.png')).toBe(true);
    expect(isSafeAssetUrl('http://x/a.png')).toBe(false);
    expect(isSafeAssetUrl('//x/a.png')).toBe(false);
    expect(isSafeAssetUrl('\\\\server\\share\\a.png')).toBe(false);
    expect(isSafeAssetUrl('blob:abc')).toBe(false);
    expect(isSafeAssetUrl('javascript:alert(1)')).toBe(false);
    expect(isSafeAssetUrl('')).toBe(false);
  });
  it('recognises svg and image extensions', () => {
    expect(isSvgSource('figures/a.svg')).toBe(true);
    expect(isSvgSource('data:image/svg+xml;utf8,<svg/>')).toBe(true);
    expect(isSvgSource('a.png')).toBe(false);
    expect(looksLikeImage('a.PNG')).toBe(true);
    expect(looksLikeImage('a.webp?x=1')).toBe(true);
    expect(looksLikeImage('a.pdf')).toBe(false);
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
