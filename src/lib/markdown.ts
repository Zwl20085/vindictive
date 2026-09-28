import { marked } from 'marked';
import DOMPurify from 'dompurify';

export type ImageResolver = (src: string) => Promise<string>;

const EXTERNAL_LINK_ATTRS = { target: '_blank', rel: 'noopener noreferrer' } as const;
const ABSOLUTE_SRC_RE = /^(https?:|data:|blob:)/i;

let hooksInstalled = false;

function installHooks(): void {
  if (hooksInstalled) return;
  hooksInstalled = true;
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    if (node.tagName === 'A' && node.hasAttribute('href')) {
      node.setAttribute('target', EXTERNAL_LINK_ATTRS.target);
      node.setAttribute('rel', EXTERNAL_LINK_ATTRS.rel);
    }
  });
}

/** Render Markdown to sanitised HTML. Synchronous. */
export function renderMarkdown(markdown: string): string {
  installHooks();
  const html = marked.parse(markdown ?? '', { async: false, gfm: true, breaks: false }) as string;
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true } });
}

export function isRelativeSrc(src: string | null): src is string {
  return !!src && !ABSOLUTE_SRC_RE.test(src.trim());
}

/**
 * Replace every relative `<img src>` inside `root` with whatever `resolve`
 * returns (normally a data URL from the backend). Failures leave the image
 * with an `alt` fallback and a `data-error` marker; nothing is swallowed.
 */
export async function resolveImages(root: ParentNode, resolve: ImageResolver): Promise<void> {
  const images = Array.from(root.querySelectorAll('img'));
  await Promise.all(
    images.map(async (img) => {
      const src = img.getAttribute('src');
      if (!isRelativeSrc(src)) return;
      img.setAttribute('data-original-src', src);
      try {
        img.setAttribute('src', await resolve(src));
      } catch (error) {
        console.error('Vindictive: failed to resolve image', src, error);
        img.setAttribute('data-error', String(error));
        img.removeAttribute('src');
      }
    }),
  );
}
