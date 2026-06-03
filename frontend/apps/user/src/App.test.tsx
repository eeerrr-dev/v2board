import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { USER_APP_LAYOUT_ROUTE_PATHS, USER_LEGACY_ROUTE_PATHS } from './App';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'App.tsx'), 'utf8');

describe('user legacy route table', () => {
  it('matches the bundled user route list exactly', () => {
    expect([...USER_LEGACY_ROUTE_PATHS]).toEqual([
      '/dashboard',
      '/forgetpassword',
      '/',
      '/invite',
      '/knowledge',
      '/login',
      '/node',
      '/order/:trade_no',
      '/order',
      '/plan/:plan_id',
      '/plan',
      '/profile',
      '/register',
      '/ticket/:ticket_id',
      '/ticket',
      '/traffic',
    ]);
  });

  it('does not expose route aliases absent from the bundled theme', () => {
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/forget');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/plans');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/orders');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/tickets');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/nodes');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/home');
  });

  it('keeps ticket details as the original standalone chat route', () => {
    expect(USER_APP_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
    expect(source).toContain(
      '<Route path="/ticket/:ticket_id" element={USER_ROUTE_ELEMENTS[\'/ticket/:ticket_id\']} />',
    );
  });

  it('redirects unmatched legacy hashes back through the bundled home route', () => {
    expect(source).toContain('<Route path="*" element={USER_ROUTE_ELEMENTS[\'/\']} />');
  });
});
