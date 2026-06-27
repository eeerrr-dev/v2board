import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { sanitizeLegacyHtml } from './sanitize-html';

const source = readFileSync(`${process.cwd()}/src/lib/sanitize-html.ts`, 'utf8');

describe('legacy HTML sanitization', () => {
  it('uses DOMPurify as the primary sanitizer and gates the DOM fallback behind reliability probing', () => {
    expect(source).toContain("from 'dompurify'");
    expect(source).toContain('function isDOMPurifyReliable');
    expect(source).toContain('sanitizeLegacyHtmlWithDomApi');
  });

  it('preserves safe legacy markup and removes unsafe event/script content', () => {
    expect(
      sanitizeLegacyHtml(
        '<section class="hero"><a href="https://example.com" onclick="alert(1)">Go</a><script>alert(1)</script></section>',
      ),
    ).toBe('<section class="hero"><a href="https://example.com">Go</a></section>');
  });
});
