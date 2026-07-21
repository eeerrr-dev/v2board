import type { InternalApiProblemDetails } from '@v2board/types';
import { z } from 'zod';
import { internalApiProblemDetailsSchema } from './generated/internal-api';

// Modern internal-dialect client core (docs/api-dialect.md §14, Appendix A
// §W0). Inert foundations: everything here is exported and unit-tested only —
// no endpoint or client path consumes it until its family's wave (W2+) flips.
// The legacy transport in client.ts stays authoritative until then.

/**
 * §4.2 — `Authorization: Bearer <auth_data>` on every authenticated internal
 * request. The localStorage `authorization` key keeps storing the raw
 * `auth_data` value (§2 frozen); the client prepends the scheme on the wire.
 */
export function bearerAuthorization(authData: string | null | undefined): string | null {
  if (!authData) return null;
  return `Bearer ${authData}`;
}

/**
 * §4.3 — `Accept-Language: <active-locale>` replaces `Content-Language` as
 * the request locale signal. The backend resolves it against the enabled
 * locale registry and localizes problem `detail`/`errors`.
 */
export function acceptLanguageHeader(locale: string | null | undefined): string | null {
  if (!locale) return null;
  return locale;
}

export interface DialectHeaderInputs {
  authData?: string | null;
  locale?: string | null;
}

/** §4.2 + §4.3 — assemble the modern transport headers for one request. */
export function dialectRequestHeaders({ authData, locale }: DialectHeaderInputs = {}): Record<
  string,
  string
> {
  const headers: Record<string, string> = {};
  const authorization = bearerAuthorization(authData);
  if (authorization) headers.Authorization = authorization;
  const acceptLanguage = acceptLanguageHeader(locale);
  if (acceptLanguage) headers['Accept-Language'] = acceptLanguage;
  return headers;
}

/**
 * §3.1 — the RFC 9457 problem body every internal-route error carries.
 * RFC 9457 extension members remain accepted, while the generated stable
 * internal `code` registry and each code's status/title invariants stay closed.
 */
export const problemDetailsSchema = internalApiProblemDetailsSchema;

export type ProblemDetails = InternalApiProblemDetails;

/**
 * The modern error surface: `{status, code, detail, errors}` (§14). `code` is
 * the frontend's only discriminator (§3.1); `detail` is presentation-only and
 * doubles as `Error#message`. No client logic may branch on `detail`.
 */
export class ApiProblemError extends Error {
  public readonly status: number;
  public readonly code: string;
  public readonly detail: string;
  public readonly errors?: Record<string, string[]>;
  public readonly raw: unknown;

  constructor(status: number, problem: ProblemDetails) {
    super(problem.detail);
    this.name = 'ApiProblemError';
    this.status = status;
    this.code = problem.code;
    this.detail = problem.detail;
    if (problem.errors) this.errors = problem.errors;
    this.raw = problem;
  }
}

/**
 * Parse a problem+json response body into the ApiError model. `status` is the
 * transport HTTP status (authoritative; §3.1 pins the body `status` to mirror
 * it). Returns null when the body is not a problem document — e.g. the legacy
 * `{message}` dialect, which stays on the legacy `ApiError` path.
 */
export function parseProblem(body: unknown, status: number): ApiProblemError | null {
  const result = problemDetailsSchema.safeParse(body);
  if (!result.success) return null;
  return new ApiProblemError(status, result.data);
}

export function isApiProblemError(error: unknown): error is ApiProblemError {
  return error instanceof ApiProblemError;
}

/** Code-first discrimination (§3.1): the slug replaces message matching. */
export function hasProblemCode(error: unknown, code: string): boolean {
  return isApiProblemError(error) && error.code === code;
}

/**
 * §3.2 — the session-teardown key: exactly 401 + `session_expired`. A 403
 * `permission_denied`/`step_up_required` must never tear the session down
 * (the 403-keep-token behavior, now keyed by code instead of message text).
 */
export function isSessionExpiredProblem(error: unknown): boolean {
  return hasProblemCode(error, 'session_expired') && (error as ApiProblemError).status === 401;
}

/**
 * §3.2/§4.2 — the privileged step-up gate, signalled by
 * `code: "step_up_required"` instead of the legacy message literal. The
 * step-up token keeps riding `x-v2board-step-up`.
 */
export function isStepUpRequiredProblem(error: unknown): boolean {
  return hasProblemCode(error, 'step_up_required') && (error as ApiProblemError).status === 403;
}

/**
 * §8/§14 — the `{items, total}` page shape replacing the legacy `{data,
 * total}` envelope. Non-paginated lists stay bare arrays; never wrap them.
 */
export const pageSchema = <TItemSchema extends z.ZodType>(item: TItemSchema) =>
  z.object({
    items: z.array(item),
    total: z.number(),
  });

// ——— §7 admin filter & sort DSL (docs/api-dialect.md §7, shipped in W9 with
// GET system/logs as its first consumer; the W11/W12 admin list waves reuse
// these builders). Replaces the legacy `filter[i][key]/[condition]/[value]`
// bracket params, the `模糊` operator token, and `sort`/`sort_type`.

/** §7.1 — the closed filter operator vocabulary. */
export const filterOpSchema = z.enum(['eq', 'neq', 'like', 'gt', 'gte', 'lt', 'lte', 'in']);
export type FilterOp = z.output<typeof filterOpSchema>;

const filterScalarSchema = z.union([z.string(), z.number(), z.boolean()]);
const filterValueSchema = z.union([
  filterScalarSchema,
  z.null(),
  // `in`: a non-empty array of scalars (§7.1).
  z.array(filterScalarSchema).min(1),
]);

/**
 * §7.1 — one filter clause against a per-endpoint field whitelist. Endpoints
 * instantiate this with their §7.1 column list so an unknown field fails in
 * the client instead of round-tripping to a 422.
 */
export const filterClauseSchema = <TField extends readonly [string, ...string[]]>(fields: TField) =>
  z.object({
    field: z.enum(fields),
    op: filterOpSchema,
    value: filterValueSchema,
  });

export interface FilterClause<TField extends string = string> {
  field: TField;
  op: FilterOp;
  value: string | number | boolean | null | Array<string | number | boolean>;
}

/** §7.2 — enum-validated sort direction (invalid values are 422s, not defaults). */
export const sortDirSchema = z.enum(['asc', 'desc']);
export type SortDir = z.output<typeof sortDirSchema>;

/** §8 + §7 — one admin list request: pagination, filter clauses, and sort. */
export interface AdminListQuery<TField extends string = string> {
  page?: number;
  per_page?: number;
  filter?: FilterClause<TField>[];
  sort_by?: string;
  sort_dir?: SortDir;
}

/**
 * §7.1/§7.2 — encode an admin list query into query params: the clause array
 * serializes with `JSON.stringify` into the single `filter` param (URL
 * encoding is the transport's job); `page`/`per_page`/`sort_by`/`sort_dir`
 * ride as plain scalars. An empty/absent clause list omits `filter` entirely.
 */
export function adminListQueryParams<TField extends string>(
  query: AdminListQuery<TField> = {},
): Record<string, string | number> {
  const params: Record<string, string | number> = {};
  if (query.page !== undefined) params.page = query.page;
  if (query.per_page !== undefined) params.per_page = query.per_page;
  if (query.filter !== undefined && query.filter.length > 0) {
    params.filter = JSON.stringify(query.filter);
  }
  if (query.sort_by !== undefined) params.sort_by = query.sort_by;
  if (query.sort_dir !== undefined) params.sort_dir = query.sort_dir;
  return params;
}
