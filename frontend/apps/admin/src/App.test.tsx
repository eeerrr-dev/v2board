import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ADMIN_LAYOUT_ROUTE_PATHS,
  ADMIN_LEGACY_ROUTE_PATHS,
  ADMIN_STANDALONE_ROUTE_PATHS,
} from './App';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'App.tsx'), 'utf8');

describe('admin legacy route table', () => {
  it('matches the bundled admin route list exactly', () => {
    expect([...ADMIN_LEGACY_ROUTE_PATHS]).toEqual([
      '/config/payment',
      '/config/system',
      '/config/theme',
      '/coupon',
      '/giftcard',
      '/dashboard',
      '/',
      '/knowledge',
      '/login',
      '/notice',
      '/order',
      '/plan',
      '/queue',
      '/server/group',
      '/server/manage',
      '/server/route',
      '/ticket/:ticket_id',
      '/ticket',
      '/user',
    ]);
  });

  it('does not expose new-admin alias routes that were absent from the bundle', () => {
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/users');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/orders');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/plans');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/servers');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/tickets');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/payments');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/coupons');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/notices');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/system');
    expect(ADMIN_LEGACY_ROUTE_PATHS).not.toContain('/stats');
  });

  it('keeps the bundled ticket chat route outside the admin layout shell', () => {
    expect([...ADMIN_STANDALONE_ROUTE_PATHS]).toEqual(['/ticket/:ticket_id']);
    expect(ADMIN_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
    expect(ADMIN_LAYOUT_ROUTE_PATHS).toContain('/ticket');
  });

  it('does not add route-level auth wrappers absent from the bundled admin routes', () => {
    expect(source).not.toContain('RequireAuth');
    expect(source).toContain(
      '<Route path="/ticket/:ticket_id" element={ADMIN_ROUTE_ELEMENTS[\'/ticket/:ticket_id\']} />',
    );
    expect(source).toContain('<AdminLayout />');
  });

  it('keeps the bundled root route redirect shape', () => {
    expect(source).toContain('function RootRedirect()');
    expect(source).toContain("navigate('/login');");
    expect(source).toContain('return <div />;');
    expect(source).toContain("'/': <RootRedirect />,");
    expect(source).not.toContain('<Navigate to="/login" />');
  });

  it('keeps the bundled admin route modules synchronous instead of adding a new route fallback', () => {
    expect(source).not.toContain('lazy(() => import(');
    expect(source).not.toContain('<Suspense');
    expect(source).not.toContain('fallback={<Fallback />}');
    expect(source).not.toContain('Spin size="large"');
    expect(source).toContain("import DashboardPage from '@/pages/dashboard';");
    expect(source).toContain("import ConfigPage from '@/pages/config';");
  });
});
