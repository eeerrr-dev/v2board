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
    // hash-routed by design), and the native-only /audit surface (the oracle
    // has no counterpart page).
    expect(
      interactions.filter((interaction) => interaction.sourceOnly).map(({ label }) => label),
    ).toEqual([
      'user-register-legacy-hash-entry',
      'admin-plan-legacy-hash-entry',
      'admin-audit-filters',
      'a11y-user-login',
      'a11y-admin-login',
      'a11y-user-dashboard',
      'a11y-admin-users',
    ]);
  });
});
