import createDOMPurify from 'dompurify';

type DOMPurifyInstance = {
  isSupported?: boolean;
  sanitize: (html: string, config?: Record<string, unknown>) => string;
};

type DOMPurifyFactory = {
  (window: Window): DOMPurifyInstance;
  sanitize?: DOMPurifyInstance['sanitize'];
};

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

const LEGACY_HTML_SANITIZE_CONFIG = {
  ALLOWED_ATTR: [...LEGACY_HTML_ALLOWED_ATTRS],
  ALLOWED_TAGS: [...LEGACY_HTML_ALLOWED_TAGS],
  ADD_ATTR: [
    'data-v2board-markdown-action',
    'data-v2board-markdown-value',
    'rel',
    'target',
  ],
  ADD_DATA_URI_TAGS: ['img'],
  ALLOW_DATA_ATTR: true,
};

export function sanitizeLegacyHtml(html: string) {
  const purify = getDOMPurify();
  if (!canSanitizeWithDOMPurify(purify)) return '';
  return purify.sanitize(html, LEGACY_HTML_SANITIZE_CONFIG);
}

function getDOMPurify() {
  const purify = createDOMPurify as unknown as DOMPurifyFactory;
  if (typeof window !== 'undefined' && typeof purify === 'function') return purify(window);
  if (typeof purify.sanitize === 'function') return purify as DOMPurifyInstance;
  if (typeof window === 'undefined') return null;
  return purify(window);
}

function canSanitizeWithDOMPurify(purify: DOMPurifyInstance | null): purify is DOMPurifyInstance {
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
