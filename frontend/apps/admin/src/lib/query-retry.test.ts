import { ApiContractError, ApiError } from '@v2board/api-client';
import { describe, expect, it } from 'vitest';
import { shouldRetryAdminQuery } from './query-retry';

describe('shouldRetryAdminQuery', () => {
  it('never retries deterministic response-contract failures', () => {
    const error = new ApiContractError('/admin/ticket/fetch', { user_id: null }, new Error());
    expect(shouldRetryAdminQuery(0, error)).toBe(false);
  });

  it('retries transport and server failures at most twice', () => {
    expect(shouldRetryAdminQuery(0, new Error('network'))).toBe(true);
    expect(shouldRetryAdminQuery(1, new ApiError(500, 'server'))).toBe(true);
    expect(shouldRetryAdminQuery(2, new ApiError(503, 'server'))).toBe(false);
  });

  it('does not retry client errors', () => {
    expect(shouldRetryAdminQuery(0, new ApiError(422, 'invalid'))).toBe(false);
  });
});
