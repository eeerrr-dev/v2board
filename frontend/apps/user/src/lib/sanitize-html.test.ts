// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { sanitizeLegacyHtml } from './sanitize-html';

const source = readFileSync(`${process.cwd()}/src/lib/sanitize-html.ts`, 'utf8');

describe('legacy HTML sanitization', () => {
  it('sanitizes through DOMPurify directly, gated only by its own support flag, with no second sanitizer or output-string probe', () => {
    expect(source).toContain("from 'dompurify'");
    expect(source).toContain('DOMPurify.isSupported');
    expect(source).toContain('DOMPurify.sanitize(html, LEGACY_HTML_SANITIZE_CONFIG)');
    expect(source).not.toContain('sanitizeLegacyHtmlWithDomApi');
    expect(source).not.toContain('function isDOMPurifyReliable');
    expect(source).not.toContain('function canSanitizeWithDOMPurify');
  });

  it('preserves safe legacy markup when supported and never returns unsafe event/script content', () => {
    const sanitized = sanitizeLegacyHtml(
      '<section class="hero"><a href="https://example.com" onclick="alert(1)">Go</a><script>alert(1)</script></section>',
    );

    expect([
      '<section class="hero"><a href="https://example.com">Go</a></section>',
      '',
    ]).toContain(sanitized);
    expect(sanitized).not.toContain('onclick');
    expect(sanitized).not.toContain('<script>');
  });
});
