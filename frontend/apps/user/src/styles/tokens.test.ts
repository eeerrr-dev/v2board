import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { tokens } from '@v2board/tokens';

// The Tailwind @theme (tokens.css) and the framework-neutral source of truth (tokens.ts) live in
// @v2board/tokens. They straddle the CSS/JS boundary, so this test locks them together in both
// directions — drift would silently desync the user app's utilities from the admin antd theme.
const css = readFileSync(`${process.cwd()}/../../packages/tokens/src/tokens.css`, 'utf8');

describe('design tokens (@v2board/tokens)', () => {
  it('declares every tokens.ts value verbatim in the tokens.css @theme', () => {
    for (const [name, value] of Object.entries(tokens)) {
      expect(css, `${name} must be declared in tokens.css`).toContain(`${name}: ${value};`);
    }
  });

  it('keeps every tokens.css custom property present in tokens.ts', () => {
    const declared = Array.from(css.matchAll(/^\s*(--[\w-]+):/gm), (match) => match[1]);
    const names = Object.keys(tokens);
    expect(declared.length).toBeGreaterThan(0);
    for (const name of declared) {
      expect(names, `${name} declared in tokens.css must exist in tokens.ts`).toContain(name);
    }
  });
});
