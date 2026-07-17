import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import {
  ApiProblemError,
  acceptLanguageHeader,
  bearerAuthorization,
  dialectRequestHeaders,
  hasProblemCode,
  isApiProblemError,
  isSessionExpiredProblem,
  isStepUpRequiredProblem,
  pageSchema,
  parseProblem,
} from './dialect';

// The docs/api-dialect.md §3.1 example body, verbatim.
const specProblemBody = {
  type: 'about:blank',
  title: 'Bad Request',
  status: 400,
  code: 'plan_sold_out',
  detail: '当前产品已售罄',
  errors: { email: ['邮箱格式不正确'] },
};

describe('bearer header assembly (§4.2)', () => {
  it('prepends the Bearer scheme to the raw stored auth_data', () => {
    expect(bearerAuthorization('abc123')).toBe('Bearer abc123');
  });

  it('sends nothing without credentials', () => {
    expect(bearerAuthorization(null)).toBeNull();
    expect(bearerAuthorization(undefined)).toBeNull();
    expect(bearerAuthorization('')).toBeNull();
  });
});

describe('Accept-Language helper (§4.3)', () => {
  it('sends the active locale', () => {
    expect(acceptLanguageHeader('ja-JP')).toBe('ja-JP');
  });

  it('sends nothing without an active locale', () => {
    expect(acceptLanguageHeader(null)).toBeNull();
    expect(acceptLanguageHeader('')).toBeNull();
  });
});

describe('dialectRequestHeaders', () => {
  it('assembles Authorization and Accept-Language together', () => {
    expect(dialectRequestHeaders({ authData: 'token-1', locale: 'zh-CN' })).toEqual({
      Authorization: 'Bearer token-1',
      'Accept-Language': 'zh-CN',
    });
  });

  it('omits absent headers instead of sending empty values', () => {
    expect(dialectRequestHeaders({})).toEqual({});
    expect(dialectRequestHeaders({ authData: null, locale: null })).toEqual({});
    expect(dialectRequestHeaders({ locale: 'en-US' })).toEqual({ 'Accept-Language': 'en-US' });
  });
});

describe('problem+json parsing (§3.1)', () => {
  it('parses the spec example into the {status, code, detail, errors} surface', () => {
    const problem = parseProblem(specProblemBody, 400);
    expect(problem).toBeInstanceOf(ApiProblemError);
    expect(problem?.name).toBe('ApiProblemError');
    expect(problem?.status).toBe(400);
    expect(problem?.code).toBe('plan_sold_out');
    expect(problem?.detail).toBe('当前产品已售罄');
    expect(problem?.errors).toEqual({ email: ['邮箱格式不正确'] });
    // detail is the human-readable message; code stays the only machine key.
    expect(problem?.message).toBe('当前产品已售罄');
  });

  it('keeps the errors bag optional (present only for validation_failed)', () => {
    const { errors: _errors, ...withoutErrors } = specProblemBody;
    const problem = parseProblem(withoutErrors, 400);
    expect(problem?.errors).toBeUndefined();
  });

  it('parses unknown codes so apps can fall back to detail verbatim', () => {
    const problem = parseProblem(
      { ...specProblemBody, code: 'some_future_code', detail: 'future detail' },
      400,
    );
    expect(problem?.code).toBe('some_future_code');
    expect(problem?.detail).toBe('future detail');
  });

  it('tolerates RFC 9457 extension members', () => {
    const problem = parseProblem({ ...specProblemBody, trace_id: 'abc' }, 400);
    expect(problem?.code).toBe('plan_sold_out');
  });

  it('returns null for the legacy {message} dialect and non-problem bodies', () => {
    expect(parseProblem({ message: 'Invalid coupon' }, 400)).toBeNull();
    expect(parseProblem({ code: 400, data: null, message: 'oops' }, 400)).toBeNull();
    expect(parseProblem('Server Error', 500)).toBeNull();
    expect(parseProblem(null, 500)).toBeNull();
    expect(parseProblem(undefined, 0)).toBeNull();
  });

  it('uses the transport status as the authoritative status', () => {
    const problem = parseProblem({ ...specProblemBody, status: 400 }, 422);
    expect(problem?.status).toBe(422);
  });
});

describe('code-first discrimination (§3.2)', () => {
  const problemOf = (status: number, code: string) =>
    parseProblem(
      { type: 'about:blank', title: 'Error', status, code, detail: 'detail' },
      status,
    ) as ApiProblemError;

  it('discriminates by code, never by message text', () => {
    const problem = problemOf(400, 'coupon_expired');
    expect(isApiProblemError(problem)).toBe(true);
    expect(hasProblemCode(problem, 'coupon_expired')).toBe(true);
    expect(hasProblemCode(problem, 'coupon_invalid')).toBe(false);
    expect(hasProblemCode(new Error('coupon_expired'), 'coupon_expired')).toBe(false);
  });

  it('keys session teardown on exactly 401 + session_expired', () => {
    expect(isSessionExpiredProblem(problemOf(401, 'session_expired'))).toBe(true);
    // 403 permission_denied / step_up_required must never tear the session down.
    expect(isSessionExpiredProblem(problemOf(403, 'permission_denied'))).toBe(false);
    expect(isSessionExpiredProblem(problemOf(403, 'step_up_required'))).toBe(false);
    expect(isSessionExpiredProblem(problemOf(403, 'session_expired'))).toBe(false);
  });

  it('detects the step-up gate by 403 + step_up_required', () => {
    expect(isStepUpRequiredProblem(problemOf(403, 'step_up_required'))).toBe(true);
    expect(isStepUpRequiredProblem(problemOf(403, 'permission_denied'))).toBe(false);
    expect(isStepUpRequiredProblem(problemOf(401, 'step_up_required'))).toBe(false);
  });
});

describe('pageSchema (§8)', () => {
  const itemSchema = z.object({ id: z.number() });

  it('parses the modern {items, total} page shape', () => {
    const page = pageSchema(itemSchema).parse({ items: [{ id: 1 }, { id: 2 }], total: 40 });
    expect(page.items).toHaveLength(2);
    expect(page.total).toBe(40);
  });

  it('rejects the legacy {data, total} page envelope', () => {
    const result = pageSchema(itemSchema).safeParse({ data: [{ id: 1 }], total: 1 });
    expect(result.success).toBe(false);
  });

  it('rejects a page without a total', () => {
    const result = pageSchema(itemSchema).safeParse({ items: [] });
    expect(result.success).toBe(false);
  });
});
