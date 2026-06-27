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
  if (!isDOMPurifyReliable(purify)) return sanitizeLegacyHtmlWithDomApi(html);
  return purify.sanitize(html, LEGACY_HTML_SANITIZE_CONFIG);
}

function getDOMPurify() {
  const purify = createDOMPurify as unknown as DOMPurifyFactory;
  if (typeof window !== 'undefined' && typeof purify === 'function') return purify(window);
  if (typeof purify.sanitize === 'function') return purify as DOMPurifyInstance;
  if (typeof window === 'undefined') return null;
  return purify(window);
}

function isDOMPurifyReliable(purify: DOMPurifyInstance | null): purify is DOMPurifyInstance {
  if (!purify || purify.isSupported === false) return false;
  if (typeof purify.sanitize !== 'function') return false;
  const probe = purify.sanitize(
    '<section class="hero"><a href="https://example.com" onclick="alert(1)">Go</a><script>alert(1)</script></section>',
    LEGACY_HTML_SANITIZE_CONFIG,
  );
  return (
    probe === '<section class="hero"><a href="https://example.com">Go</a></section>' ||
    probe === '<section class="hero"><a href="https://example.com">Go</a></section>\n'
  );
}

const allowedTags = new Set<string>(LEGACY_HTML_ALLOWED_TAGS);
const allowedAttrs = new Set<string>(LEGACY_HTML_ALLOWED_ATTRS);
const removedWithContent = new Set(['iframe', 'object', 'script', 'style']);

function sanitizeLegacyHtmlWithDomApi(html: string) {
  if (typeof document === 'undefined') return '';
  const template = document.createElement('template');
  template.innerHTML = html;
  sanitizeChildNodes(template.content);
  return template.innerHTML;
}

function sanitizeChildNodes(parent: ParentNode) {
  for (const node of Array.from(parent.childNodes)) {
    if (node.nodeType === Node.COMMENT_NODE) {
      node.remove();
      continue;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) continue;
    const element = node as HTMLElement;
    const tag = element.tagName.toLowerCase();
    if (!allowedTags.has(tag)) {
      if (removedWithContent.has(tag)) {
        element.remove();
        continue;
      }
      element.replaceWith(...Array.from(element.childNodes));
      sanitizeChildNodes(parent);
      continue;
    }
    sanitizeAttributes(element);
    sanitizeChildNodes(element);
  }
}

function sanitizeAttributes(element: HTMLElement) {
  for (const attr of Array.from(element.attributes)) {
    const name = attr.name.toLowerCase();
    if (
      name === 'style' ||
      name.startsWith('on') ||
      !allowedAttrs.has(name) ||
      !isSafeAttributeValue(element, name, attr.value)
    ) {
      element.removeAttribute(attr.name);
    }
  }
}

function isSafeAttributeValue(element: HTMLElement, name: string, value: string) {
  if (name !== 'href' && name !== 'src') return true;
  const normalized = value.trim().replace(/[\u0000-\u001F\u007F\s]+/g, '').toLowerCase();
  if (!normalized) return true;
  if (
    normalized.startsWith('#') ||
    normalized.startsWith('/') ||
    normalized.startsWith('./') ||
    normalized.startsWith('../')
  ) {
    return true;
  }
  if (!/^[a-z][a-z0-9+.-]*:/i.test(normalized)) return true;
  if (/^(https?:|mailto:|tel:)/.test(normalized)) return true;
  return element.tagName.toLowerCase() === 'img' && normalized.startsWith('data:image/');
}
