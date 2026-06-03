import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { describe, expect, it } from 'vitest';
import { renderLegacyMarkdown } from './markdown';

type LegacyRequire = {
  (id: string): unknown;
  n(module: unknown): { a: new (options: Record<string, unknown>) => { render(source: string): string } };
};

describe('legacy markdown rendering', () => {
  const renderOldMarkdown = createOldBundleMarkdownRenderer();

  it.each([
    '# Title',
    'https://example.com?a=1&b=2',
    'foo@example.com',
    '"quote" -- test...',
    '<div class="x">ok</div>',
    '| a | b |\n| - | - |\n| 1 | 2 |',
    '~~del~~',
    '- a\n- b',
    '[x](javascript:alert(1))',
    '![x](data:image/png;base64,aaa)',
  ])('matches the packaged theme for %s', (source) => {
    expect(renderLegacyMarkdown(source)).toBe(renderOldMarkdown(source));
  });
});

function createOldBundleMarkdownRenderer() {
  const assetsRoot = findLegacyAssetsRoot();
  const sandbox = {
    window: { webpackJsonp: [] as unknown[] },
    console,
    setTimeout,
    clearTimeout,
  } as {
    window: { webpackJsonp: unknown[]; __old_require__?: LegacyRequire };
    console: Console;
    setTimeout: typeof setTimeout;
    clearTimeout: typeof clearTimeout;
    self?: unknown;
    global?: unknown;
  };
  sandbox.self = sandbox.window;
  sandbox.global = sandbox;

  for (const file of ['vendors.async.js', 'components.async.js']) {
    vm.runInNewContext(readFileSync(path.join(assetsRoot, file), 'utf8'), sandbox, {
      filename: file,
    });
  }

  let umiSource = readFileSync(path.join(assetsRoot, 'umi.js'), 'utf8');
  umiSource = umiSource.replace('a.p = "./";', 'a.p = "./"; window.__old_require__ = a;');
  umiSource = umiSource.replace(/i\.push\(\[1, 2, 0\]\),\s*n\(\)/, 'void 0');
  vm.runInNewContext(umiSource, sandbox, { filename: 'umi.js' });

  const legacyRequire = sandbox.window.__old_require__;
  if (!legacyRequire) throw new Error('Failed to load legacy webpack runtime');
  const MarkdownIt = legacyRequire.n(legacyRequire('1M3H')).a;
  const md = new MarkdownIt({ html: true, linkify: true, typographer: true });
  return (source: string) => md.render(source);
}

function findLegacyAssetsRoot() {
  const starts = [process.cwd(), path.dirname(fileURLToPath(import.meta.url))];
  for (const start of starts) {
    let current = path.resolve(start);
    while (true) {
      const candidate = path.join(current, 'public/theme/default/assets');
      if (existsSync(path.join(candidate, 'umi.js'))) return candidate;
      const parent = path.dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }
  throw new Error('Could not find public/theme/default/assets');
}
