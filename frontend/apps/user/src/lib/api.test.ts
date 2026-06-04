import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

describe('user api unauthorized handling', () => {
  it('clears auth and redirects to the site root on 403 like the bundled theme', () => {
    expect(apiSource).toContain('logout();');
    expect(apiSource).toContain("window.location.href = '/';");
    expect(apiSource).not.toContain('redirectingToLogin');
    expect(apiSource).not.toContain("window.location.replace('/#/login');");
  });
});
