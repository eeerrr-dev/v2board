import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// knip/depcheck do not follow CSS `@import`, so dead stylesheets accumulate
// silently. The user runtime intentionally ships no Bootstrap/OneUI framework
// CSS; this walks the `@import` graph rooted at the stylesheets main.tsx loads
// and asserts every styles/*.css is actually reachable, failing the moment an
// orphaned stylesheet is reintroduced.
const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '..');
const stylesDir = resolve(srcDir, 'styles');
const mainPath = resolve(srcDir, 'main.tsx');

function localCssImports(css: string): string[] {
  return [...css.matchAll(/@import\s+['"](\.[^'"]+\.css)['"]/g)].map((match) => match[1]!);
}

function entryStylesheetsFromMain(): string[] {
  const main = readFileSync(mainPath, 'utf8');
  return [...main.matchAll(/import\s+['"](\.\/styles\/[^'"]+\.css)['"]/g)].map((match) =>
    resolve(srcDir, match[1]!),
  );
}

describe('user stylesheet reachability', () => {
  it('every styles/*.css is reachable from the stylesheets main.tsx imports', () => {
    const reachable = new Set<string>();
    const stack = entryStylesheetsFromMain();
    expect(stack.length).toBeGreaterThan(0);

    while (stack.length > 0) {
      const file = stack.pop()!;
      if (reachable.has(file)) continue;
      reachable.add(file);
      for (const imported of localCssImports(readFileSync(file, 'utf8'))) {
        stack.push(resolve(dirname(file), imported));
      }
    }

    const orphans = readdirSync(stylesDir)
      .filter((name) => name.endsWith('.css'))
      .map((name) => resolve(stylesDir, name))
      .filter((file) => !reachable.has(file))
      .map((file) => relative(stylesDir, file))
      .sort();

    expect(orphans).toEqual([]);
  });
});
