import axios, {
  AxiosHeaders,
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
  type AxiosResponse,
} from 'axios';
import type { output, ZodType } from 'zod';
import { bearerAuthorization, isSessionExpiredProblem, parseProblem } from './dialect';
import type { ApiProblemError } from './dialect';

export class ApiError extends Error {
  public readonly status: number;
  public readonly raw?: unknown;

  constructor(status: number, message: string, raw?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.raw = raw;
  }
}

export class ApiContractError extends Error {
  public readonly endpoint: string;
  public readonly raw: unknown;

  constructor(endpoint: string, raw: unknown, cause: unknown) {
    super(`API response does not match its contract: ${endpoint}`, { cause });
    this.name = 'ApiContractError';
    this.endpoint = endpoint;
    this.raw = raw;
  }
}

/**
 * §3.2 — fired exactly once per session-teardown error: 401 + problem code
 * `session_expired`. A 403 `permission_denied`/`step_up_required` is an
 * authorization verdict for a live session and never reaches this hook.
 */
export interface ApiUnauthorizedHook {
  (error: ApiProblemError): void;
}

export interface ApiClientOptions {
  baseURL?: string;
  /** Default request deadline. Individual requests may override it with Axios `timeout`. */
  timeoutMs?: number;
  /** Opt in only for deployments that deliberately authenticate with cross-origin cookies. */
  withCredentials?: boolean;
  getAuthData?: () => string | null;
  getLocale?: () => string | null;
  /**
   * Current privileged step-up token (POST /passport/auth/stepUp), sent as the
   * `x-v2board-step-up` header. Return null once the grant's `expires_in` has
   * elapsed: the backend rejects a stale token outright instead of falling
   * back to the recent-password window.
   */
  getStepUpToken?: () => string | null;
  onUnauthorized?: ApiUnauthorizedHook;
  adminSecurePath?: () => string | null;
}

export type ApiRequestConfig = AxiosRequestConfig & {
  /**
   * docs/api-dialect.md §4.1/§14 — the internal-dialect wave marker. Since
   * W14 closed the wave series, v2 is the only internal dialect: JSON object
   * bodies (never form-encoded), real HTTP success statuses (200/201/204 all
   * pass), and a bare-body response with no legacy envelope. The marker stays
   * as per-endpoint documentation of the §5–§6 route tables.
   */
  dialect?: 'v2';
  /**
   * Exact success status (or finite exact status set) declared by the
   * endpoint contract. Axios still accepts every 2xx at the transport
   * boundary so the client can surface a status drift as an ApiContractError
   * before applying the response schema.
   */
  expectedStatus?: number | readonly number[];
};

export type JsonApiRequestConfig<TSchema extends ZodType> = Omit<
  ApiRequestConfig,
  'responseType'
> & {
  responseSchema: TSchema;
  responseType?: 'json';
};

export type BinaryApiRequestConfig<TJsonSchema extends ZodType> = Omit<
  ApiRequestConfig,
  'responseType'
> & {
  /** Schema for the JSON envelope returned when the CSV-capable endpoint has no file. */
  jsonResponseSchema: TJsonSchema;
};

export interface RawBinaryResponse {
  code: number;
  data: ArrayBuffer;
  buffer: ArrayBuffer;
}

/** Narrow the explicit CSV/binary arm returned by a mixed-content operation. */
export function isRawBinaryResponse(value: unknown): value is RawBinaryResponse {
  return (
    typeof value === 'object' &&
    value !== null &&
    'buffer' in value &&
    value.buffer instanceof ArrayBuffer
  );
}

export type BinaryApiResponse<TJsonSchema extends ZodType> =
  RawBinaryResponse | output<TJsonSchema>;

export interface ApiClient {
  axios: AxiosInstance;
  /** Validates and returns the bare dialect success body (§14). */
  request: <TSchema extends ZodType>(
    config: JsonApiRequestConfig<TSchema>,
  ) => Promise<output<TSchema>>;
  /** Explicit escape hatch for endpoints that may return either CSV bytes or JSON. */
  requestBinary: <TJsonSchema extends ZodType>(
    config: BinaryApiRequestConfig<TJsonSchema>,
  ) => Promise<BinaryApiResponse<TJsonSchema>>;
  resolveAdminPath: (path: string) => string;
}

export function createApiClient(options: ApiClientOptions = {}): ApiClient {
  const instance = axios.create({
    baseURL: options.baseURL ?? '/api/v1',
    timeout: options.timeoutMs ?? 15_000,
    withCredentials: options.withCredentials ?? false,
    // §4.1: success is any real 2xx — 200 reads, 201 creates, 204 bodiless.
    validateStatus: (status) => status >= 200 && status < 300,
  });

  instance.interceptors.request.use((config) => {
    const locale = options.getLocale?.();
    config.headers = config.headers ?? {};
    // §4.2: every internal request carries the Bearer scheme on the wire;
    // the auth store keeps holding the raw auth_data value.
    const authorization = bearerAuthorization(options.getAuthData?.());
    if (authorization) {
      config.headers.authorization = authorization;
    }
    const stepUpToken = options.getStepUpToken?.();
    if (stepUpToken) config.headers['x-v2board-step-up'] = stepUpToken;
    if (locale) {
      // docs/api-dialect.md §4.3: Accept-Language is the request locale
      // signal, resolved against the enabled locale registry. (The W1→W14
      // transitional Content-Language copy died when W14 closed the wave
      // series — no internal route localizes `message` bodies anymore.)
      config.headers['Accept-Language'] = locale;
    }
    if (config.params) {
      // §4.1: query values are plain scalars; array params ride as repeated
      // keys (`reply_status=0&reply_status=1`) — no legacy bracket encoding.
      const query = serializeQuery(config.params);
      if (query) {
        const separator = config.url?.includes('?') ? '&' : '?';
        config.url = `${config.url}${separator}${query}`;
      }
      config.params = undefined;
    }
    return config;
  });

  instance.interceptors.response.use(
    (response) => normalizeArrayBufferJsonResponse(response),
    (error: AxiosError<{ errors?: Record<string, string[]>; message?: string }>) => {
      const status = error.response?.status ?? 0;
      // §3.1: problem+json is discriminated by its stable `code` slug and is
      // the only internal error dialect since W14; the `{message}` fallback
      // below covers non-dialect failures (gateway HTML, network bodies).
      const problem = parseProblem(error.response?.data, status);
      if (problem) {
        // §3.2: exactly 401 + `session_expired` tears the session down. 403
        // `permission_denied`/`step_up_required` are live-session verdicts
        // and must never end the session.
        if (isSessionExpiredProblem(problem)) {
          options.onUnauthorized?.(problem);
        }
        return Promise.reject(problem);
      }
      const message =
        firstValidationError(error.response?.data?.errors) ??
        error.response?.data?.message ??
        error.message ??
        'Request failed, please try again later';
      return Promise.reject(new ApiError(status, message, error.response?.data));
    },
  );

  return {
    axios: instance,
    request: async <TSchema extends ZodType>(config: JsonApiRequestConfig<TSchema>) => {
      const { responseSchema, expectedStatus, ...requestConfig } = config;
      const response = await instance.request<unknown>(requestConfig);
      const endpoint = String(config.url ?? '<unknown>');
      assertExpectedSuccessStatus(response, expectedStatus, endpoint);
      // §14: the dialect response is the bare success body — nothing to
      // unwrap. A bodiless 204 (axios yields '' or undefined) parses as
      // undefined against the endpoint's empty-success schema.
      const body = response.data === '' || response.data == null ? undefined : response.data;
      return parseContract(responseSchema, body, endpoint);
    },
    requestBinary: async <TJsonSchema extends ZodType>(
      config: BinaryApiRequestConfig<TJsonSchema>,
    ) => {
      const { jsonResponseSchema, expectedStatus, ...requestConfig } = config;
      const response = await instance.request<unknown>({
        ...requestConfig,
        responseType: 'arraybuffer',
      });
      const endpoint = String(config.url ?? '<unknown>');
      assertExpectedSuccessStatus(response, expectedStatus, endpoint);
      const buffer = toArrayBuffer(response.data);
      if (buffer) return { code: response.status, data: buffer, buffer };
      // §14: the dialect JSON arm is the bare success body (e.g. the §1
      // 201 `{id}` create) — no envelope to unwrap; the CSV arm above is
      // byte-frozen either way.
      return parseContract(jsonResponseSchema, response.data, endpoint);
    },
    resolveAdminPath: (path) => {
      const securePath = options.adminSecurePath?.();
      if (!securePath) return path;
      return `/${securePath}${path}`;
    },
  };
}

function assertExpectedSuccessStatus(
  response: AxiosResponse<unknown>,
  expectedStatus: number | readonly number[] | undefined,
  endpoint: string,
): void {
  const accepted =
    expectedStatus === undefined ||
    (typeof expectedStatus === 'number'
      ? response.status === expectedStatus
      : expectedStatus.includes(response.status));
  if (accepted) return;
  const expected =
    typeof expectedStatus === 'number' ? String(expectedStatus) : expectedStatus.join(' or ');
  throw new ApiContractError(
    endpoint,
    response.data,
    new TypeError(`Expected HTTP ${expected}, received ${response.status}`),
  );
}

function parseContract<TSchema extends ZodType>(
  schema: TSchema,
  value: unknown,
  endpoint: string,
): output<TSchema> {
  const result = schema.safeParse(value);
  if (!result.success) throw new ApiContractError(endpoint, value, result.error);
  return result.data;
}

function normalizeArrayBufferJsonResponse<T>(response: AxiosResponse<T>): AxiosResponse<T> {
  if (response.config.responseType !== 'arraybuffer') return response;
  if (getContentType(response.headers) !== 'application/json') return response;
  const buffer = toArrayBuffer(response.data);
  if (!buffer) return response;
  const text = new TextDecoder().decode(buffer);
  response.data = (text ? JSON.parse(text) : null) as T;
  return response;
}

function getContentType(headers: AxiosResponse['headers']): string {
  const value =
    headers instanceof AxiosHeaders ? headers.get('content-type') : headers['content-type'];
  return typeof value === 'string' ? value : String(value ?? '');
}

function toArrayBuffer(data: unknown): ArrayBuffer | null {
  if (data instanceof ArrayBuffer) return data;
  if (!ArrayBuffer.isView(data)) return null;
  const view = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  return view.slice().buffer;
}

function firstValidationError(errors: Record<string, string[]> | undefined): string | undefined {
  if (!errors) return undefined;
  return Object.values(errors)[0]?.[0];
}

/**
 * §4.1 query serialization — the only wire the internal dialect speaks:
 * scalar params as plain `key=value` pairs, array params as repeated keys,
 * nullish values omitted. (The legacy recursive bracket form serializer died
 * with W14.)
 */
function serializeQuery(params: unknown): string {
  if (!params || typeof params !== 'object' || Array.isArray(params)) return '';
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue;
    if (Array.isArray(value)) {
      for (const item of value) {
        if (item !== undefined && item !== null) query.append(key, String(item));
      }
      continue;
    }
    query.append(key, String(value));
  }
  return query.toString();
}
