import MarkdownIt from 'markdown-it';
import { sanitizeLegacyHtml } from './sanitize-html';

export const LEGACY_MARKDOWN_ACTION_ATTRIBUTE = 'data-v2board-markdown-action';
export const LEGACY_MARKDOWN_VALUE_ATTRIBUTE = 'data-v2board-markdown-value';

type LegacyMarkdownAction = {
  action: 'copy' | 'jump';
  value: string;
};

// The packaged theme renders knowledge bodies with markdown-it configured exactly as
// `new MarkdownIt({ html: true, linkify: true, typographer: true })` and injects the
// result via dangerouslySetInnerHTML. Using the
// same engine + options is the only way to render byte-identically — a hand-rolled parser
// only approximates markdown-it's grammar, linkify, and typographer rules.
const md = new MarkdownIt({ html: true, linkify: true, typographer: true });

export function renderLegacyMarkdown(markdown: string) {
  return sanitizeLegacyHtml(preserveLegacyMarkdownActions(md.render(markdown)));
}

function preserveLegacyMarkdownActions(html: string) {
  if (typeof document === 'undefined') return html;
  const template = document.createElement('template');
  template.innerHTML = html;
  template.content.querySelectorAll<HTMLElement>('[onclick]').forEach((element) => {
    const action = parseLegacyMarkdownAction(element.getAttribute('onclick') ?? '');
    if (!action) return;
    element.removeAttribute('onclick');
    element.setAttribute(LEGACY_MARKDOWN_ACTION_ATTRIBUTE, action.action);
    element.setAttribute(LEGACY_MARKDOWN_VALUE_ATTRIBUTE, action.value);
    if (!element.matches('a[href], button, input, select, textarea, [tabindex]')) {
      element.setAttribute('tabindex', '0');
    }
    if (!element.matches('a[href], button, [role]')) element.setAttribute('role', 'button');
  });
  return template.innerHTML;
}

function parseLegacyMarkdownAction(expression: string): LegacyMarkdownAction | null {
  const match = /^(copy|jump)\((.*)\);?$/.exec(expression.trim());
  if (!match) return null;
  return {
    action: match[1] as LegacyMarkdownAction['action'],
    value: parseLegacyMarkdownActionValue(match[2] ?? ''),
  };
}

function parseLegacyMarkdownActionValue(value: string) {
  const trimmed = value.trim();
  const quoted = /^(['"])(.*)\1$/.exec(trimmed);
  if (!quoted) return trimmed;
  return (quoted[2] ?? '')
    .replace(/\\n/g, '\n')
    .replace(/\\r/g, '\r')
    .replace(/\\t/g, '\t')
    .replace(/\\(["'\\])/g, '$1');
}
