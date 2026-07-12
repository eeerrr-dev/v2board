import { beforeEach, describe, expect, it } from 'vitest';
import { takeStoredAdminFilters } from './stored-admin-filters';

const KEY = 'stored-admin-filter-test';

describe('takeStoredAdminFilters', () => {
  beforeEach(() => window.sessionStorage.clear());

  it('returns and consumes a valid AdminFilter array', () => {
    const filters = [{ key: 'status', condition: 'is', value: 1 }];
    window.sessionStorage.setItem(KEY, JSON.stringify(filters));

    expect(takeStoredAdminFilters(KEY)).toEqual(filters);
    expect(window.sessionStorage.getItem(KEY)).toBeNull();
  });

  it.each([
    ['null item', [null]],
    ['string item', ['status']],
    ['obsolete item shape', [{ key: 'status' }]],
    ['non-array shape', { key: 'status', condition: 'is', value: 1 }],
  ])('discards and consumes a malformed %s', (_label, value) => {
    window.sessionStorage.setItem(KEY, JSON.stringify(value));

    expect(takeStoredAdminFilters(KEY)).toEqual([]);
    expect(window.sessionStorage.getItem(KEY)).toBeNull();
  });
});
