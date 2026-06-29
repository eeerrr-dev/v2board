import { describe, expect, it } from 'vitest';
import { getSafeTosHref } from './auth-tos-field';

describe('getSafeTosHref', () => {
  it.each([
    ['https://terms.example', 'https://terms.example'],
    ['http://terms.example/path?q=1', 'http://terms.example/path?q=1'],
    ['  https://terms.example  ', 'https://terms.example'],
    ['/terms', '/terms'],
    ['/legal/tos', '/legal/tos'],
  ])('keeps the safe href %j', (input, expected) => {
    expect(getSafeTosHref(input)).toBe(expected);
  });

  it.each([
    ['', 'empty'],
    ['   ', 'whitespace only'],
    ['//evil.example', 'protocol-relative'],
    ['/\\evil.example', 'backslash protocol-relative'],
    ['/\u0009/evil.example', 'tab-injected protocol-relative'],
    ['javascript:alert(1)', 'javascript scheme'],
    ['data:text/html,<x>', 'data scheme'],
    ['mailto:a@b.com', 'mailto scheme'],
    ['ftp://files.example', 'non-http scheme'],
  ])('rejects %j (%s)', (input) => {
    expect(getSafeTosHref(input)).toBeNull();
  });
});
