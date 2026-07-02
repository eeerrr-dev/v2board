// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { sanitizeLegacyHtml } from './sanitize-html';

describe('legacy HTML sanitization', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('preserves safe legacy markup and strips event handlers and script content', () => {
    const sanitized = sanitizeLegacyHtml(
      '<section class="hero"><a href="https://example.com" onclick="alert(1)">Go</a><script>alert(1)</script></section>',
    );

    expect(sanitized).toContain('<section class="hero">');
    expect(sanitized).toContain('href="https://example.com"');
    expect(sanitized).toContain('>Go</a>');
    expect(sanitized).not.toContain('onclick');
    expect(sanitized).not.toContain('<script');
    expect(sanitized).not.toContain('alert');
  });

  it('keeps the markdown action hooks and rich markup the knowledge surface relies on', () => {
    const sanitized = sanitizeLegacyHtml(
      '<div><button data-v2board-markdown-action="copy" data-v2board-markdown-value="text">Copy</button>' +
        '<table><tbody><tr><td colspan="2">cell</td></tr></tbody></table>' +
        '<img src="data:image/png;base64,AAAA" alt="qr" /></div>',
    );

    expect(sanitized).toContain('data-v2board-markdown-action="copy"');
    expect(sanitized).toContain('data-v2board-markdown-value="text"');
    expect(sanitized).toContain('colspan="2"');
    expect(sanitized).toContain('data:image/png;base64,AAAA');
  });

  it('drops disallowed embeds, form controls, and javascript: URLs', () => {
    const sanitized = sanitizeLegacyHtml(
      '<form action="/steal"><input name="a" /></form>' +
        '<iframe src="https://evil.example"></iframe>' +
        '<a href="javascript:alert(1)">bad</a>',
    );

    expect(sanitized).not.toContain('<form');
    expect(sanitized).not.toContain('<input');
    expect(sanitized).not.toContain('<iframe');
    expect(sanitized).not.toContain('javascript:');
  });

  it('fails closed to an empty string when no DOM window exists', () => {
    vi.stubGlobal('window', undefined);
    expect(sanitizeLegacyHtml('<p>content</p>')).toBe('');
  });
});
