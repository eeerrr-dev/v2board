export { createApiClient, ApiContractError, ApiError } from './client';
export type {
  ApiClient,
  ApiClientOptions,
  ApiRequestConfig,
  ApiUnauthorizedHook,
  BackendEnvelope,
  BinaryApiRequestConfig,
  BinaryApiResponse,
  JsonApiRequestConfig,
  RawBinaryResponse,
} from './client';
// Modern internal-dialect core (docs/api-dialect.md §14, Appendix A §W0):
// inert foundations, consumed family-by-family from W2 on.
export {
  ApiProblemError,
  acceptLanguageHeader,
  adminListQueryParams,
  bearerAuthorization,
  dialectRequestHeaders,
  filterClauseSchema,
  filterOpSchema,
  hasProblemCode,
  isApiProblemError,
  isSessionExpiredProblem,
  isStepUpRequiredProblem,
  pageSchema,
  parseProblem,
  problemDetailsSchema,
  sortDirSchema,
} from './dialect';
export type {
  AdminListQuery,
  DialectHeaderInputs,
  FilterClause,
  FilterOp,
  ProblemDetails,
  SortDir,
} from './dialect';
export { decimalToCents, decimalToMinorUnits, decimalToScaledInteger } from './money';
export {
  getErrorPresentation,
  INLINE_MUTATION_ERROR_META,
  presentMutationError,
  shouldRetryQuery,
} from './error-presentation';
export type { ErrorPresentation, MutationErrorMeta } from './error-presentation';
export { adminFilterArraySchema } from './endpoints/admin';
export * as passport from './endpoints/passport';
export * as guest from './endpoints/guest';
export * as user from './endpoints/user';
export * as admin from './endpoints/admin';
export type { AdminFilter, AdminPageQuery, AdminUserUpdateInput } from './endpoints/admin';
