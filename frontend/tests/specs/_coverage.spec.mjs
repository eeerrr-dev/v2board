import { test, expect } from '@playwright/test';
import { interactions } from '../lib/interaction-scenarios.mjs';
import { SOURCE_ONLY_INTERACTION_ALLOWLIST } from '../lib/interaction-contract.mjs';
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

  test('legacy-incomparable interactions are limited to the explicit allowlist', () => {
    // These interactions skip only the optional legacy comparison, so each one
    // must be a deliberate exception: the axe gate (the oracle is not a valid a11y
    // baseline), the §10.3 hash-entry boot translator (the oracle is
    // hash-routed by design), the native-only /audit surface (the oracle has
    // no counterpart page), and the §6.12 staff RBAC pair (the oracle
    // predates staff grants).
    expect(
      interactions.filter((interaction) => interaction.sourceOnly).map(({ label }) => label),
    ).toEqual(SOURCE_ONLY_INTERACTION_ALLOWLIST);
  });
});
