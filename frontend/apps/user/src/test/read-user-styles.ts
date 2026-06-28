import { readFileSync } from 'node:fs';

export const userStyleFiles = [
  'globals.css',
  'user-theme-colors.css',
  'user-theme-legacy-tokens.css',
  'user-theme-layout-tokens.css',
  'user-document-root.css',
  'user-heading-base.css',
  'user-heading-scale.css',
  'user-heading-native-color.css',
  'user-prose-elements.css',
  'user-link-elements.css',
  'user-custom-html-base.css',
  'user-custom-html-headings.css',
  'user-custom-html-inline.css',
  'user-custom-html-lists.css',
  'user-custom-html-divider.css',
  'user-custom-html-code-block.css',
  'user-custom-html-inline-code.css',
  'user-custom-html-blockquote.css',
  'user-custom-html-media.css',
  'user-custom-html-table-shell.css',
  'user-custom-html-table-cell-wrap.css',
  'user-custom-html-table-rows.css',
  'user-custom-html-table-header-cells.css',
  'user-custom-html-table-body-cells.css',
  'user-browser-modes.css',
  'user-shadcn.css',
  'user-shadcn-motion.css',
  'user-auth-surface.css',
] as const;

export function readUserStyles() {
  return userStyleFiles.map((file) => readFileSync(`src/styles/${file}`, 'utf8')).join('');
}
