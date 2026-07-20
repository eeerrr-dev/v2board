import { test, expect } from '@playwright/test';
import { interactions } from '../lib/interaction-scenarios.mjs';
import { GROUP_NAMES, groupOf } from '../lib/spec-groups.mjs';

// A structural guard, not a browser test: it fails fast if a newly added
// interaction lands in no spec group (and would silently never run).
test.describe('parity: coverage', () => {
  // Playwright 1.61 requires an object-destructured fixture argument even when
  // this project-only predicate intentionally consumes no fixtures.
  // eslint-disable-next-line no-empty-pattern
  test.skip(({}, testInfo) => testInfo.project.name === 'mobile', 'run once');

  test('every interaction maps to exactly one spec group', () => {
    const unmapped = interactions.filter((interaction) => groupOf(interaction) === null);
    expect(unmapped.map((interaction) => interaction.label)).toEqual([]);

    const perGroup = GROUP_NAMES.reduce((total, name) => {
      return total + interactions.filter((interaction) => groupOf(interaction) === name).length;
    }, 0);
    expect(perGroup).toBe(interactions.length);
  });

  test('source-only interactions are limited to the explicit allowlist', () => {
    // Source-only runs skip the cross-world oracle comparison, so each one must
    // be a deliberate exception: the axe gate (the oracle is not a valid a11y
    // baseline), the §10.3 hash-entry boot translator (the oracle is
    // hash-routed by design), the native-only /audit surface (the oracle has
    // no counterpart page), and the §6.12 staff RBAC pair (the oracle
    // predates staff grants).
    expect(
      interactions.filter((interaction) => interaction.sourceOnly).map(({ label }) => label),
    ).toEqual([
      'user-register-legacy-hash-entry',
      'admin-plan-legacy-hash-entry',
      'admin-audit-filters',
      'admin-users-staff-permissions',
      'admin-staff-session-gating',
      'a11y-user-login',
      'a11y-admin-login',
      'a11y-user-dashboard',
      'a11y-admin-users',
      'a11y-user-register',
      'a11y-user-forget',
      'a11y-user-plans',
      'a11y-user-plan-checkout',
      'a11y-user-orders',
      'a11y-user-node',
      'a11y-user-traffic',
      'a11y-user-invite',
      'a11y-user-tickets',
      'a11y-user-knowledge',
      'a11y-user-profile',
      'a11y-admin-config',
      'a11y-admin-plans',
      'a11y-admin-server-manage',
      'a11y-admin-orders',
      'a11y-admin-coupons',
      'a11y-admin-notices',
      'a11y-admin-tickets',
      'a11y-admin-audit',
    ]);
  });
});
