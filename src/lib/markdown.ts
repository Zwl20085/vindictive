import { marked } from 'marked';
import DOMPurify from 'dompurify';

export type ImageResolver = (src: string) => Promise<string>;

const EXTERNAL_LINK_ATTRS = { target: '_blank', rel: 'noopener noreferrer' } as const;
const ABSOLUTE_SRC_RE = /^(https?:|data:|blob:)/i;

/** Any `scheme:` prefix. */
const SCHEME_RE = /^[a-z][a-z0-9+.-]*:/i;
/** Attributes that make the webview fetch something as soon as the HTML is shown. */
const LOADING_ATTRS = ['src', 'href', 'xlink:href', 'poster', 'background'] as const;
/** Elements whose `href` is a link the user has to click, not a fetch. */
const LINK_ELEMENTS = new Set(['a', 'area']);
/** `SANITIZE_NAMED_PROPS` puts this in front of every `id` and `name`. */
const NAMED_PROP_PREFIX = 'user-content-';
const URL_REF_RE = /url\(\s*(['"]?)#(?!user-content-)/g;

/**
 * Point in-document references (`url(#grad)` in SVG, `href="#sec"`) at the
 * prefixed ids DOMPurify writes, so gradients, markers and filters keep working.
 */
function followPrefixedIds(node: Element): void {
  for (const attr of Array.from(node.attributes)) {
    const { name, value } = attr;
    let next = value;
    if ((name === 'href' || name === 'xlink:href') && value.startsWith('#') && !value.startsWith(`#${NAMED_PROP_PREFIX}`) && value.length > 1) {
      next = `#${NAMED_PROP_PREFIX}${value.slice(1)}`;
    } else if (value.includes('url(')) {
      next = value.replace(URL_REF_RE, `url($1#${NAMED_PROP_PREFIX}`);
    }
    if (next !== value) node.setAttribute(name, next);
  }
}

/**
 * Whether a body asset may load: a path relative to the tip (resolved later
 * through the backend), an in-page `#fragment`, a `data:` URL or an `https:`
 * URL (the tips folder belongs to one user, and the CSP already limits
 * img-src to https: and data:). Plain http, protocol-relative `//host`,
 * UNC `\\server` and every other scheme are dropped.
 */
export function isSafeAssetUrl(value: string): boolean {
  const v = value.trim();
  if (!v) return false;
  if (/^(data|https):/i.test(v)) return true;
  if (/^[/\\]{2}/.test(v)) return false;
  return !SCHEME_RE.test(v);
}

let hooksInstalled = false;

function installHooks(): void {
  if (hooksInstalled) return;
  hooksInstalled = true;
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    const name = node.nodeName.toLowerCase();
    if (name === 'a' && node.hasAttribute('href')) {
      node.setAttribute('target', EXTERNAL_LINK_ATTRS.target);
      node.setAttribute('rel', EXTERNAL_LINK_ATTRS.rel);
    }
    followPrefixedIds(node);
    // srcset lists several URLs; a body never needs one, so it goes outright.
    node.removeAttribute('srcset');
    for (const attr of LOADING_ATTRS) {
      if (LINK_ELEMENTS.has(name) && (attr === 'href' || attr === 'xlink:href')) continue;
      const value = node.getAttribute(attr);
      if (value !== null && !isSafeAssetUrl(value)) node.removeAttribute(attr);
    }
  });
}

/**
 * Render Markdown to sanitised HTML. Synchronous. Inline `<svg>` is kept
 * (scripts, event handlers, foreignObject and style are stripped) so a tip
 * body can carry a small hand-drawn figure without a separate file. Auto-loading
 * URLs other than relative, `data:` and `https:` ones are removed.
 */
export function renderMarkdown(markdown: string): string {
  installHooks();
  const html = marked.parse(markdown ?? '', { async: false, gfm: true, breaks: false }) as string;
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true, svg: true, svgFilters: true },
    FORBID_TAGS: ['foreignObject', 'style'],
    FORBID_ATTR: ['style'],
    ALLOW_DATA_ATTR: false,
    SANITIZE_NAMED_PROPS: true,
  });
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

const IMAGE_EXT_RE = /\.(svg|png|jpe?g|gif|webp|bmp)(\?.*)?$/i;

export function isSvgSource(src: string): boolean {
  return /\.svg(\?.*)?$/i.test(src.trim()) || /^data:image\/svg\+xml/i.test(src.trim());
}

export function looksLikeImage(src: string): boolean {
  return IMAGE_EXT_RE.test(src.trim()) || /^data:image\//i.test(src.trim());
}
