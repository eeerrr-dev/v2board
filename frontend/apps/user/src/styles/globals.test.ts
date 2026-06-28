import { describe, expect, it } from 'vitest';
import { readUserStyles } from '../test/read-user-styles';

const css = readUserStyles;

describe('custom HTML content CSS', () => {
  it('keeps rich announcement and knowledge article content readable', () => {
    const globals = css();

    expect(globals).toContain('.custom-html-style {\n  color: #333;\n}');
    expect(globals).toContain('.custom-html-style h1 {\n  font-size: 32px;');
    expect(globals).toContain('.custom-html-style p {\n  font-size: 14px;\n  line-height: 1.7;');
    expect(globals).toContain('.custom-html-style a {\n  color: #0052d9;\n}');
    expect(globals).toContain('.custom-html-style pre {\n  display: block;');
    expect(globals).toContain('.custom-html-style blockquote {\n  position: relative;');
    expect(globals).toContain('.custom-html-style table {\n  font-size: 14px;');
    expect(globals).toContain('.custom-html-style table td {\n  border: 1px solid #efefef;');
  });
});

describe('remaining legacy utility CSS boundary', () => {
  it('keeps rich-content compatibility without Bootstrap or OneUI UI foundations', () => {
    const globals = css();

    expect(globals).toContain('--color-brand-500: #0665d0;');
    expect(globals).toContain('--color-page: #f0f3f8;');
    expect(globals).toContain('.custom-html-style table th {');
    expect(globals).toContain('::selection {');

    expect(globals).not.toContain('.btn {');
    expect(globals).not.toContain('.form-control {');
    expect(globals).not.toContain('.block {');
    expect(globals).not.toContain('.row {');
    expect(globals).not.toContain('.col-md-');
    expect(globals).not.toContain('.bg-white {');
    expect(globals).not.toContain('.ant-btn {');
    expect(globals).not.toContain('.ant-table {');
    expect(globals).not.toContain('.ant-select {');
    expect(globals).not.toContain('.ant-tooltip {');
    expect(globals).not.toContain('.ant-drawer {');
    expect(globals).not.toContain('.ant-switch {');
    expect(globals).not.toContain('.ant-carousel {');
    expect(globals).not.toContain('.am-list-item {');
    expect(globals).not.toContain('.slick-dots');
    expect(globals).not.toContain('.v2board-background {');
    expect(globals).not.toContain('.v2board-auth-lang-btn {');
    expect(globals).not.toContain('.sidebar-mini.sidebar-o');
    expect(globals).not.toContain('.nav-main');
  });
});

describe('shadcn island presentation CSS', () => {
  it('declares the pure shadcn island theme variables and source boundaries', () => {
    const globals = css();

    expect(globals).toContain("@import 'tailwindcss' prefix(tw);");
    expect(globals).toContain("@import 'tailwindcss/theme.css';");
    expect(globals).toContain('@media important {\n  @tailwind utilities source(none);');
    expect(globals).not.toContain("@import 'tailwindcss';");
    expect(globals).toContain('@theme inline {');
    expect(globals).toContain('--color-card: var(--card);');
    expect(globals).toContain('--color-muted-foreground: var(--muted-foreground);');
    expect(globals).toContain("@source '../pages/auth/**/*.tsx';");
    expect(globals).toContain("@source '../pages/dashboard.tsx';");
    expect(globals).toContain("@source '../components/ui/**/*.tsx';");
    expect(globals).toContain('.v2board-auth-surface,');
    expect(globals).toContain('.v2board-app-shell,');
    expect(globals).toContain('.v2board-radix-dialog-content {\n  --radius: 0.625rem;');
    expect(globals).toContain('--card: oklch(1 0 0);');
    expect(globals).toContain('--background: oklch(1 0 0);');
    expect(globals).toContain('--primary: oklch(0.205 0 0);');
  });

  it('keeps shadcn motion and class-driven dark theme tokens', () => {
    const globals = css();

    expect(globals).toContain('.v2board-page-shell {\n  animation: v2board-page-in 180ms ease-out both;');
    expect(globals).toContain('color-mix(in oklch, var(--muted) 42%, transparent)');
    expect(globals).not.toContain('hsl(var(--background))');
    expect(globals).not.toContain('@media (prefers-color-scheme: dark)');
    expect(globals).toContain('.dark .v2board-auth-surface,');
    expect(globals).toContain('.dark .v2board-app-shell,');
    expect(globals).toContain('color-scheme: dark;');
    expect(globals).toContain('--background: oklch(0.145 0 0);');
    expect(globals).toContain('--card: oklch(0.205 0 0);');
    expect(globals).toContain('--muted-foreground: oklch(0.708 0 0);');
  });

  it('themes portaled shadcn feedback without legacy login or Ant chrome', () => {
    const globals = css();

    expect(globals).toContain('.v2board-auth-toast-icon-success {');
    expect(globals).toContain('.dark .v2board-auth-toast-root,');
    expect(globals).toContain('.dark .v2board-auth-language-menu-content,');
    expect(globals).toContain('.dark .v2board-app-shell-menu-content,');
    expect(globals).toContain('.dark .v2board-radix-dialog-content {');
    expect(globals).not.toContain('.v2board-login-i18n-btn {');
    expect(globals).not.toContain('.ant-modal {');
    expect(globals).not.toContain('.ant-message {');
    expect(globals).not.toContain('.ant-notification {');
  });
});
