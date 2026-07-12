import { describe, expect, it } from 'vitest';
import { readUserStyles } from '../test/read-user-styles';

describe('user CSS system', () => {
  it('uses one canonical Tailwind preflight and production-only source discovery', () => {
    const css = readUserStyles();

    expect(css.match(/@import 'tailwindcss\/theme\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tailwindcss\/preflight\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tailwindcss\/utilities\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tw-animate-css'/g)).toHaveLength(1);
    expect(css).toContain('layer(utilities) source(none);');
    expect(css).toContain("@source '../**/*.{ts,tsx}';");
    expect(css).toContain("@source not '../**/*.{test,spec}.{ts,tsx}';");
    expect(css).toContain("@source not '../test/**';");
    expect(css).not.toContain('@tailwind utilities');
    expect(css).not.toContain("@import 'tailwindcss' source(none);");
    expect(css).not.toContain('prefix(tw)');
    expect(css).not.toContain('v2board-radix-');
  });

  it('owns global shadcn tokens without an island-scoped preflight', () => {
    const css = readUserStyles();

    expect(css).toContain(':root {\n  --radius: 0.625rem;');
    expect(css).toContain('.dark {\n  --background: oklch(0.145 0 0);');
    expect(css).toContain(":root[data-theme-color='green']");
    expect(css).toContain('@apply border-border outline-ring/50;');
    expect(css).toContain('@apply bg-background text-foreground;');
    expect(css).not.toContain('Island-scoped Preflight');
  });

  it('scopes backend-authored prose and removes global legacy element rules', () => {
    const css = readUserStyles();

    expect(css).toContain('.custom-html-style h1 {');
    expect(css).toContain('.custom-html-style :where(p, ul, ol, pre, blockquote, table) {');
    expect(css).toContain('.custom-html-style table th {');
    expect(css).toContain('color: var(--foreground);');
    expect(css).not.toContain('.dark .custom-html-style');
    expect(css).not.toContain('\nh1,\nh2,\nh3,');
    expect(css).not.toContain('a:not([class])');
    expect(css).not.toContain('\nsmall {');
    expect(css).not.toContain('--legacy-link');
    expect(css).not.toContain('--color-page');
  });

  it('contains no Bootstrap, OneUI, or Ant presentation foundation', () => {
    const css = readUserStyles();

    for (const selector of [
      '.btn {',
      '.form-control {',
      '.block {',
      '.row {',
      '.col-md-',
      '.ant-btn {',
      '.ant-table {',
      '.ant-select {',
      '.ant-tooltip {',
      '.ant-drawer {',
      '.ant-switch {',
    ]) {
      expect(css).not.toContain(selector);
    }
  });
});
