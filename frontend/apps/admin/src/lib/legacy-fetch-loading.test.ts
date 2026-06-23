import { describe, expect, it } from 'vitest';
import { legacyFetchLoading } from './legacy-fetch-loading';

describe('legacyFetchLoading', () => {
  it('keeps old admin loading masks visible for transport failures only', () => {
    expect(legacyFetchLoading(true)).toBe(true);
    expect(legacyFetchLoading(false, { status: 0 })).toBe(true);
    expect(legacyFetchLoading(false, { status: 500 })).toBe(false);
    expect(legacyFetchLoading(false, new Error('boom'))).toBe(false);
  });
});
