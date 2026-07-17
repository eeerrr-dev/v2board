import { describe, expect, it, vi } from 'vitest';
import { ApiContractError, ApiError } from './client';
import { ApiProblemError } from './dialect';
import {
  INLINE_MUTATION_ERROR_META,
  presentMutationError,
  shouldRetryQuery,
} from './error-presentation';

describe('mutation error presentation', () => {
  it.each([
    ['business-rule failure', new ApiError(400, 'business failed')],
    ['HTTP failure', new ApiError(502, 'upstream failed')],
    ['transport failure', new ApiError(0, 'Network Error')],
  ])('notifies exactly once for a %s', (_label, error) => {
    const notify = vi.fn();

    expect(presentMutationError(error, undefined, notify)).toBe(true);
    expect(notify).toHaveBeenCalledOnce();
    expect(notify).toHaveBeenCalledWith(error.message);
  });

  it('does not toast a mutation whose error is rendered inline', () => {
    const notify = vi.fn();

    expect(
      presentMutationError(
        new ApiError(422, 'validation failed'),
        INLINE_MUTATION_ERROR_META,
        notify,
      ),
    ).toBe(false);
    expect(notify).not.toHaveBeenCalled();
  });

  it('does not toast the session-expiry problem because the API client owns redirect teardown', () => {
    const notify = vi.fn();
    const problem = new ApiProblemError(401, {
      type: 'about:blank',
      title: 'Unauthorized',
      status: 401,
      code: 'session_expired',
      detail: '未登录或登陆已过期',
    });

    expect(presentMutationError(problem, undefined, notify)).toBe(false);
    expect(notify).not.toHaveBeenCalled();
  });

  it('does not toast 403 authorization verdicts', () => {
    const notify = vi.fn();

    expect(presentMutationError(new ApiError(403, 'Permission denied'), undefined, notify)).toBe(
      false,
    );
    expect(notify).not.toHaveBeenCalled();
  });
});

describe('shouldRetryQuery', () => {
  it('never retries deterministic response-contract failures', () => {
    const error = new ApiContractError('/admin/ticket/fetch', { user_id: null }, new Error());
    expect(shouldRetryQuery(0, error)).toBe(false);
  });

  it('retries transport and server failures at most twice', () => {
    expect(shouldRetryQuery(0, new Error('network'))).toBe(true);
    expect(shouldRetryQuery(1, new ApiError(500, 'server'))).toBe(true);
    expect(shouldRetryQuery(2, new ApiError(503, 'server'))).toBe(false);
  });

  it('does not retry client errors', () => {
    expect(shouldRetryQuery(0, new ApiError(422, 'invalid'))).toBe(false);
    // Deterministic business-rule failures are HTTP 400 in Rust and must not
    // be retried as transient.
    expect(shouldRetryQuery(0, new ApiError(400, 'Current product is sold out'))).toBe(false);
  });
});
