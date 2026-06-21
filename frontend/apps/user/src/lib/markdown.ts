import MarkdownIt from 'markdown-it';

// The packaged theme renders knowledge bodies with markdown-it configured exactly as
// `new MarkdownIt({ html: true, linkify: true, typographer: true })` and injects the
// result via dangerouslySetInnerHTML. Using the
// same engine + options is the only way to render byte-identically — a hand-rolled parser
// only approximates markdown-it's grammar, linkify, and typographer rules.
const md = new MarkdownIt({ html: true, linkify: true, typographer: true });

export function renderLegacyMarkdown(markdown: string) {
  return md.render(markdown);
}
