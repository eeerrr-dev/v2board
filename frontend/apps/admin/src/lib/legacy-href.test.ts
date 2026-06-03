import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { legacyHref } from './legacy-href';

function walk(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      files.push(...walk(path));
    } else if (path.endsWith('.tsx') && !path.endsWith('.test.tsx')) {
      files.push(path);
    }
  }
  return files;
}

describe('admin legacy javascript hrefs', () => {
  it('sets literal javascript hrefs through refs like the packaged React 16 app', () => {
    const anchor = document.createElement('a');
    legacyHref()(anchor);
    expect(anchor.getAttribute('href')).toBe('javascript:void(0);');

    const noSemicolon = document.createElement('a');
    legacyHref('javascript:void(0)')(noSemicolon);
    expect(noSemicolon.getAttribute('href')).toBe('javascript:void(0)');

    const expressionHref = document.createElement('a');
    legacyHref('javascript:(0);')(expressionHref);
    expect(expressionHref.getAttribute('href')).toBe('javascript:(0);');
  });

  it('does not pass javascript href strings through React props in admin source', () => {
    const offenders = walk(join(process.cwd(), 'src')).filter((path) => {
      const source = readFileSync(path, 'utf8');
      return /href=(["'])javascript:/.test(source);
    });

    expect(offenders).toEqual([]);
  });
});
