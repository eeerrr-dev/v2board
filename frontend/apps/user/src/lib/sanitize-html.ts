import DOMPurify, { type Config } from 'dompurify';

const BACKEND_HTML_ALLOWED_ATTRS = [
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

const BACKEND_HTML_ALLOWED_TAGS = [
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
const BACKEND_HTML_SANITIZE_CONFIG = {
  ALLOWED_ATTR: [...BACKEND_HTML_ALLOWED_ATTRS],
  ALLOWED_TAGS: [...BACKEND_HTML_ALLOWED_TAGS],
  ADD_DATA_URI_TAGS: ['img'],
  ALLOW_DATA_ATTR: true,
} satisfies Config;

// DOMPurify's default export is an instance already bound to the ambient window
// at import time. This app is CSR-only (createRoot, no SSR), so every caller runs
// in a real browser or jsdom; the single node-safety guard below fails closed only
// if the module is ever imported without a DOM.
export function sanitizeBackendHtml(html: string) {
  if (typeof window === 'undefined') return '';
  return DOMPurify.sanitize(html, BACKEND_HTML_SANITIZE_CONFIG);
}
