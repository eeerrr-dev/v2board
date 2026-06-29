import DOMPurify, { type Config } from 'dompurify';

const LEGACY_HTML_ALLOWED_ATTRS = [
  'align',
  'alt',
  'aria-label',
  'aria-hidden',
  'class',
  'colspan',
  'data-v2board-markdown-action',
  'data-v2board-markdown-value',
  'height',
  'href',
  'id',
  'rel',
  'role',
  'rowspan',
  'src',
  'style',
  'tabindex',
  'target',
  'title',
  'width',
] as const;

const LEGACY_HTML_ALLOWED_TAGS = [
  'a',
  'abbr',
  'article',
  'b',
  'blockquote',
  'br',
  'button',
  'caption',
  'code',
  'del',
  'div',
  'em',
  'figcaption',
  'figure',
  'footer',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'header',
  'hr',
  'i',
  'img',
  'ins',
  'li',
  'main',
  'mark',
  'nav',
  'ol',
  'p',
  'pre',
  's',
  'section',
  'small',
  'span',
  'strong',
  'sub',
  'sup',
  'table',
  'tbody',
  'td',
  'tfoot',
  'th',
  'thead',
  'tr',
  'u',
  'ul',
] as const;

// `target`, `rel`, and the data-v2board-* hooks are already granted by ALLOWED_ATTR
// (combined with ALLOW_DATA_ATTR), so no ADD_ATTR escape hatch is needed.
const LEGACY_HTML_SANITIZE_CONFIG = {
  ALLOWED_ATTR: [...LEGACY_HTML_ALLOWED_ATTRS],
  ALLOWED_TAGS: [...LEGACY_HTML_ALLOWED_TAGS],
  ADD_DATA_URI_TAGS: ['img'],
  ALLOW_DATA_ATTR: true,
} satisfies Config;

// DOMPurify's default export is an instance already bound to the ambient window
// at import time, so there is nothing to verify beyond its own support flag:
// with a real DOM it sanitizes per ALLOWED_TAGS/ALLOWED_ATTR, and with no DOM
// (SSR, or import before a window exists) we fail closed to ''. The positive
// support decision is memoized so later renders skip the re-check; a negative
// result is never cached, so the check re-runs once a working DOM appears.
let domPurifySupported = false;

export function sanitizeLegacyHtml(html: string) {
  if (!isDOMPurifySupported()) return '';
  return DOMPurify.sanitize(html, LEGACY_HTML_SANITIZE_CONFIG);
}

function isDOMPurifySupported(): boolean {
  if (domPurifySupported) return true;
  if (typeof window === 'undefined' || DOMPurify.isSupported === false) return false;
  domPurifySupported = true;
  return true;
}
