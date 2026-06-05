import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

describe('user api unauthorized handling', () => {
  it('clears auth and redirects once to the hash login route on 403', () => {
    expect(apiSource).toContain('logout();');
    expect(apiSource).toContain('let redirectingToLogin = false;');
    expect(apiSource).toContain('if (redirectingToLogin) return;');
    expect(apiSource).toContain('redirectingToLogin = true;');
    expect(apiSource).toContain(
      'window.location.href = `${window.location.origin}${window.location.pathname}#/login`;',
    );
    expect(apiSource).not.toContain("window.location.href = '/';");
    expect(apiSource).not.toContain("window.location.replace('/#/login');");
  });
});
