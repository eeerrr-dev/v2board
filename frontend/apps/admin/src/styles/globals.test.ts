import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const stylesDir = dirname(fileURLToPath(import.meta.url));
const sharedStylesDir = join(stylesDir, '../../../../packages/ui/src/styles');
const css = [stylesDir, sharedStylesDir]
  .flatMap((directory) =>
    readdirSync(directory)
      .filter((name) => name.endsWith('.css'))
      .map((name) => readFileSync(join(directory, name), 'utf8')),
  )
  .join('\n');

describe('admin CSS system', () => {
  it('uses canonical Tailwind/shadcn globals and production source discovery', () => {
    expect(css.match(/@import 'tailwindcss\/theme\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tailwindcss\/preflight\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tailwindcss\/utilities\.css'/g)).toHaveLength(1);
    expect(css.match(/@import 'tw-animate-css'/g)).toHaveLength(1);
    expect(css).toContain('layer(utilities) source(none);');
    expect(css).toContain("@source '../**/*.{ts,tsx}';");
    expect(css).toContain("@source not '../**/*.{test,spec}.{ts,tsx}';");
    expect(css).not.toContain('@tailwind utilities');
    expect(css).not.toContain("@import 'tailwindcss' source(none);");
    expect(css).toContain(':root {\n  --radius: 0.625rem;');
    expect(css).toContain('.dark {\n  --background: oklch(0.145 0 0);');
    expect(css).toContain('--chart-1: oklch(');
    expect(css).toContain('--color-chart-5: var(--chart-5);');
    expect(css).not.toContain('Island-scoped Preflight');
    expect(css).not.toContain('v2board-radix-');
  });

  it('does not ship legacy themes or icon fonts', () => {
    expect(existsSync(join(stylesDir, 'themes'))).toBe(false);
    expect(existsSync(join(stylesDir, 'static'))).toBe(false);
    expect(css).not.toContain('.ant-btn');
    expect(css).not.toContain('.form-control');
    expect(css).not.toContain('.block {');
  });
});
