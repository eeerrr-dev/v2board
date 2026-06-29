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

// The capability probe runs a full extra sanitize pass, so cache the first
// instance that passes it and reuse it for every later render instead of
// re-probing on each call.
let memoizedPurify: typeof DOMPurify | null = null;

export function sanitizeLegacyHtml(html: string) {
  const purify = resolveSupportedDOMPurify();
  if (!purify) return '';
  return purify.sanitize(html, LEGACY_HTML_SANITIZE_CONFIG);
}

function resolveSupportedDOMPurify(): typeof DOMPurify | null {
  if (memoizedPurify) return memoizedPurify;
  const purify = getDOMPurify();
  // A null/unverified result (window undefined at import time, or a DOMPurify
  // that fails the fail-closed probe) is intentionally NOT cached, so the check
  // re-runs once a working DOM/instance becomes available.
  if (!canSanitizeWithDOMPurify(purify)) return null;
  memoizedPurify = purify;
  return purify;
}

function getDOMPurify(): typeof DOMPurify | null {
  // The default export is a DOMPurify instance already bound to the ambient
  // window at import (and carries .sanitize/.isSupported). Fail closed to null
  // only when there is no DOM at all.
  if (typeof window === 'undefined') return null;
  return DOMPurify;
}

function canSanitizeWithDOMPurify(purify: typeof DOMPurify | null): purify is typeof DOMPurify {
  if (!purify || purify.isSupported === false || typeof purify.sanitize !== 'function') return false;
  const probe = purify.sanitize(
    '<section class="hero"><a href="https://example.com" onclick="alert(1)">Go</a><script>alert(1)</script></section>',
    LEGACY_HTML_SANITIZE_CONFIG,
  );
  return (
    probe === '<section class="hero"><a href="https://example.com">Go</a></section>' ||
    probe === '<section class="hero"><a href="https://example.com">Go</a></section>\n'
  );
}
