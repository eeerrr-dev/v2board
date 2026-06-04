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
    expect(source).toContain('path="/ticket/:ticket_id"');
    expect(source).toContain("USER_ROUTE_ELEMENTS['/ticket/:ticket_id']");
  });

  it('normalizes unmatched legacy hashes without rendering the bundled home route first', () => {
    expect(source).toContain('path="*"');
    expect(source).toContain('function LegacyUnknownRouteRedirect()');
    expect(source).toContain('getNormalizedLegacyHashPath(current, USER_LEGACY_ROUTE_OPTIONS)');
    expect(source).toContain('navigate(normalized, { replace: true });');
    expect(source).toContain('<LegacyUnknownRouteRedirect />');
  });

  it('keeps the route table stable while wrapping standalone routes with the white-screen guard', () => {
    expect(source).toContain("import { RouteBoundaryElement } from '@/components/route-error-boundary';");
    expect(source).toContain('<RouteBoundaryElement>{USER_ROUTE_ELEMENTS');
    expect(source).not.toContain('lazy(() => import(');
    expect(source).not.toContain('<Suspense');
  });

  it('keeps the shared layout mounted while switching routes', () => {
    expect(source).toContain('<Route element={<GuestLayout />}>');
    expect(source).toContain('<Route element={<AppLayout />}>');
    expect(source).not.toContain('key={routeComponentKey');
    expect(source).not.toContain('function KeyedAppLayout');
    expect(source).not.toContain('function KeyedGuestLayout');
  });
});
