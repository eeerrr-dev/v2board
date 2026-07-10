import { test, expect } from '@playwright/test';
import { interactions } from '../lib/interaction-scenarios.mjs';
import { GROUP_NAMES, groupOf } from '../lib/spec-groups.mjs';

// A structural guard, not a browser test: it fails fast if a newly added
// interaction lands in no spec group (and would silently never run).
test.describe('parity: coverage', () => {
  test.skip(({ browserName }, testInfo) => testInfo.project.name === 'mobile', 'run once');

  test('every interaction maps to exactly one spec group', () => {
    const unmapped = interactions.filter((interaction) => groupOf(interaction) === null);
    expect(unmapped.map((interaction) => interaction.label)).toEqual([]);

    const perGroup = GROUP_NAMES.reduce((total, name) => {
      return total + interactions.filter((interaction) => groupOf(interaction) === name).length;
    }, 0);
    expect(perGroup).toBe(interactions.length);
  });
});
