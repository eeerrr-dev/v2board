import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

describe('user api unauthorized handling', () => {
  it('clears stale auth and redirects once to the hash login route on 403', () => {
    expect(apiSource).not.toContain('logout();');
    expect(apiSource).toContain('let redirectingToLogin = false;');
    expect(apiSource).toContain('if (redirectingToLogin) return;');
    expect(apiSource).toContain('redirectingToLogin = true;');
    expect(apiSource).toContain('if (getAuthData() !== null) setAuthData(null);');
    expect(apiSource).toContain("window.location.hash = '#/login';");
    expect(apiSource).toContain('redirectingToLogin = false;');
    expect(apiSource).not.toContain('LEGACY_AUTH_STORAGE_KEY');
    expect(apiSource).not.toContain('window.localStorage.setItem');
    expect(apiSource).not.toContain('window.location.pathname}#/login');
    expect(apiSource).not.toContain('window.location.href = `${window.location.origin}/#/login`;');
    expect(apiSource).not.toContain("window.location.href = '/';");
    expect(apiSource).not.toContain("window.location.replace('/#/login');");
  });
});

describe('user api global error toast', () => {
  it('stays silent on any transport failure and toasts other non-200 responses', () => {
    // The packaged user frontend used fetch, which rejected before its toast code ran, so it
    // surfaced nothing for transport errors (timeout or network) — not just timeouts.
    expect(apiSource).toContain('if (error.status === 0) return;');
    expect(apiSource).toContain(
      "toast.error(i18nGet('请求失败'), { description: error.message });",
    );
    expect(apiSource).not.toContain('isLegacyTimeoutError');
    expect(apiSource).not.toContain('/timeout/i.test');
  });
});
