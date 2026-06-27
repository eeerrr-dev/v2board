import { describe, expect, it } from 'vitest';
import { renderLegacyMarkdown } from './markdown';

describe('legacy markdown rendering', () => {
  it.each([
    ['# Title', '<h1>Title</h1>\n'],
    [
      'https://example.com?a=1&b=2',
      '<p><a href="https://example.com?a=1&amp;b=2">https://example.com?a=1&amp;b=2</a></p>\n',
    ],
    ['foo@example.com', '<p><a href="mailto:foo@example.com">foo@example.com</a></p>\n'],
    ['"quote" -- test...', '<p>“quote” – test…</p>\n'],
    ['<div class="x">ok</div>', '<div class="x">ok</div>'],
    [
      '| a | b |\n| - | - |\n| 1 | 2 |',
      '<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n</tbody>\n</table>\n',
    ],
    ['~~del~~', '<p><s>del</s></p>\n'],
    ['- a\n- b', '<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n'],
    ['[x](javascript:alert(1))', '<p>[x](javascript:alert(1))</p>\n'],
    [
      '![x](data:image/png;base64,aaa)',
      '<p><img src="data:image/png;base64,aaa" alt="x"></p>\n',
    ],
  ])('matches the legacy markdown-it output for %s', (source, expected) => {
    expect(renderLegacyMarkdown(source)).toBe(expected);
  });

  it('removes unsafe markdown html while preserving safe markup', () => {
    expect(renderLegacyMarkdown('<img src=x onerror="alert(1)"><script>alert(1)</script>')).toBe(
      '<p><img src="x"></p>\n',
    );
  });

  it('translates legacy copy and jump inline handlers into safe React action hooks', () => {
    expect(
      renderLegacyMarkdown(
        '<button onclick="copy(`token`)">Copy</button><span onclick="jump(2)">Jump</span>',
      ),
    ).toBe(
      '<p><button data-v2board-markdown-action="copy" data-v2board-markdown-value="token">Copy</button><span data-v2board-markdown-action="jump" data-v2board-markdown-value="2" tabindex="0" role="button">Jump</span></p>\n',
    );
  });
});
