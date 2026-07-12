import { describe, expect, it, vi } from 'vitest';
import { ApiError } from './client';
import { INLINE_MUTATION_ERROR_META, presentMutationError } from './error-presentation';

describe('mutation error presentation', () => {
  it.each([
    ['HTTP 200 business failure', new ApiError(500, 'business failed')],
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

  it('does not toast a 403 because the API client owns redirect teardown', () => {
    const notify = vi.fn();

    expect(presentMutationError(new ApiError(403, 'expired'), undefined, notify)).toBe(false);
    expect(notify).not.toHaveBeenCalled();
  });
});
