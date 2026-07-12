import MarkdownIt from 'markdown-it';
import { sanitizeBackendHtml } from './sanitize-html';

export const BACKEND_MARKDOWN_ACTION_ATTRIBUTE = 'data-v2board-markdown-action';
export const BACKEND_MARKDOWN_VALUE_ATTRIBUTE = 'data-v2board-markdown-value';

type BackendMarkdownAction = {
  action: 'copy' | 'jump';
  value: string;
};

// Knowledge bodies are backend-authored Markdown with embedded HTML plus the
// `copy()`/`jump()` action contract. Keep markdown-it's complete grammar and
// normalize those inline actions into safe delegated React hooks before sanitizing.
const md = new MarkdownIt({ html: true, linkify: true, typographer: true });

export function renderBackendMarkdown(markdown: string) {
  return sanitizeBackendHtml(normalizeBackendMarkdownActions(md.render(markdown)));
}

function normalizeBackendMarkdownActions(html: string) {
  if (typeof document === 'undefined') return html;
  const template = document.createElement('template');
  template.innerHTML = html;
  template.content.querySelectorAll<HTMLElement>('[onclick]').forEach((element) => {
    const action = parseBackendMarkdownAction(element.getAttribute('onclick') ?? '');
    if (!action) return;
    element.removeAttribute('onclick');
    element.setAttribute(BACKEND_MARKDOWN_ACTION_ATTRIBUTE, action.action);
    element.setAttribute(BACKEND_MARKDOWN_VALUE_ATTRIBUTE, action.value);
    if (!element.matches('a[href], button, input, select, textarea, [tabindex]')) {
      element.setAttribute('tabindex', '0');
    }
    if (!element.matches('a[href], button, [role]')) element.setAttribute('role', 'button');
  });
  return template.innerHTML;
}

function parseBackendMarkdownAction(expression: string): BackendMarkdownAction | null {
  const match = /^(copy|jump)\((.*)\);?$/.exec(expression.trim());
  if (!match) return null;
  return {
    action: match[1] as BackendMarkdownAction['action'],
    value: parseBackendMarkdownActionValue(match[2] ?? ''),
  };
}

const BACKEND_MARKDOWN_ESCAPES: Record<string, string> = { n: '\n', r: '\r', t: '\t' };

function parseBackendMarkdownActionValue(value: string) {
  const trimmed = value.trim();
  const quoted = /^(['"`])(.*)\1$/.exec(trimmed);
  if (!quoted) return trimmed;
  // Single left-to-right pass matching the backend inline-action string contract:
  // `\\` consumes both characters before a following n/r/t can be
  // misread as an escape, and unknown escapes drop the backslash (`\z` -> `z`).
  return (quoted[2] ?? '').replace(
    /\\(.)/g,
    (_, char: string) => BACKEND_MARKDOWN_ESCAPES[char] ?? char,
  );
}
