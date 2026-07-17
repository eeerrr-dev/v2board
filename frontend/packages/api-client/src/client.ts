import axios, {
  AxiosHeaders,
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
  type AxiosResponse,
} from 'axios';
import type { output, ZodType } from 'zod';
import { ApiProblemError, bearerAuthorization, isSessionExpiredProblem, parseProblem } from './dialect';

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
  nullFormValue?: 'omit' | 'empty';
}

export type ApiRequestConfig = AxiosRequestConfig & {
  /**
   * docs/api-dialect.md §4.1/§14 — marks a migrated internal-dialect request:
   * JSON object bodies (never form-encoded), real HTTP success statuses
   * (200/201/204 all pass), and a bare-body response with no legacy envelope.
   */
  dialect?: 'v2';
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

export interface BackendEnvelope<T> {
  code: number;
  data: T;
  total?: number;
  type?: number;
  message?: string;
}

export interface RawBinaryResponse {
  code: number;
  data: ArrayBuffer;
  buffer: ArrayBuffer;
}

export type BinaryApiResponse<TJsonSchema extends ZodType> =
  RawBinaryResponse | output<TJsonSchema>;

type BackendEnvelopeObject = Record<string, unknown> & {
  code?: number;
  total?: number;
  type?: number;
  message?: string;
};

export interface ApiClient {
  axios: AxiosInstance;
  /** Validates and returns the backend envelope's `data` field. */
  request: <TSchema extends ZodType>(
    config: JsonApiRequestConfig<TSchema>,
  ) => Promise<output<TSchema>>;
  /** Validates and returns the complete normalized backend envelope. */
  requestEnvelope: <TSchema extends ZodType>(
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
    validateStatus: (status) => status === 200,
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
      // signal, resolved against the enabled locale registry.
      config.headers['Accept-Language'] = locale;
      // Transitional (2026-07-17, W1→W14): the legacy internal routes still
      // localize `message` bodies through the backend's Content-Language
      // response-rewrite middleware until each endpoint family migrates to
      // the dialect (see the §4.3 transition footnote in docs/api-dialect.md).
      // Delete this line together with that middleware.
      config.headers['Content-Language'] = locale;
    }
    if (config.params) {
      const query = serializeForm(config.params, options.nullFormValue ?? 'omit');
      if (query) {
        const separator = config.url?.includes('?') ? '&' : '?';
        config.url = `${config.url}${separator}${query}`;
      }
      config.params = undefined;
    }
    if ((config as ApiRequestConfig).dialect === 'v2') {
      // §4.1: dialect bodies stay JSON (axios' default object serialization)
      // and success is any real 2xx — 201 register, 204 bodiless actions.
      config.validateStatus = (status) => status >= 200 && status < 300;
    } else if (isPostRequest(config.method) && config.data === undefined) {
      config.headers['Content-Type'] = 'application/x-www-form-urlencoded';
      config.data = '';
    } else if (shouldFormEncode(config.data)) {
      config.headers['Content-Type'] = 'application/x-www-form-urlencoded';
      config.data = serializeForm(config.data, options.nullFormValue ?? 'omit');
    }
    return config;
  });

  instance.interceptors.response.use(
    (response) => normalizeArrayBufferJsonResponse(response),
    (error: AxiosError<{ errors?: Record<string, string[]>; message?: string }>) => {
      const status = error.response?.status ?? 0;
      // §3.1: problem+json is discriminated by its stable `code` slug. The
      // shared Rust session middleware emits it on every internal route —
      // including families whose success bodies are still legacy — so this
      // branch runs ahead of the legacy `{message}` mapping.
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
      const { responseSchema, ...requestConfig } = config;
      const response = await instance.request<unknown>(requestConfig);
      const endpoint = String(config.url ?? '<unknown>');
      if (config.dialect === 'v2') {
        // §14: the dialect response is the bare success body — nothing to
        // unwrap. A bodiless 204 (axios yields '' or undefined) parses as
        // undefined against the endpoint's empty-success schema.
        const body = response.data === '' || response.data == null ? undefined : response.data;
        return parseContract(responseSchema, body, endpoint);
      }
      const data = unwrapBackendEnvelope(response.data, response.status, endpoint).data;
      return parseContract(responseSchema, data, endpoint);
    },
    requestEnvelope: async <TSchema extends ZodType>(config: JsonApiRequestConfig<TSchema>) => {
      const { responseSchema, ...requestConfig } = config;
      const response = await instance.request<unknown>(requestConfig);
      const endpoint = String(config.url ?? '<unknown>');
      const envelope = unwrapBackendEnvelope(response.data, response.status, endpoint);
      return parseContract(responseSchema, envelope, endpoint);
    },
    requestBinary: async <TJsonSchema extends ZodType>(
      config: BinaryApiRequestConfig<TJsonSchema>,
    ) => {
      const { jsonResponseSchema, ...requestConfig } = config;
      const response = await instance.request<unknown>({
        ...requestConfig,
        responseType: 'arraybuffer',
      });
      const buffer = toArrayBuffer(response.data);
      if (buffer) return { code: response.status, data: buffer, buffer };
      const endpoint = String(config.url ?? '<unknown>');
      const envelope = unwrapBackendEnvelope(response.data, response.status, endpoint);
      return parseContract(jsonResponseSchema, envelope, endpoint);
    },
    resolveAdminPath: (path) => {
      const securePath = options.adminSecurePath?.();
      if (!securePath) return path;
      return `/${securePath}${path}`;
    },
  };
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

// Rust delivers failures as real HTTP statuses (handled by the response
// interceptor); the in-body `code` handling below exists for the parity
// fixtures, which reply HTTP 200 with `{ code: 400, ... }` envelopes.
function unwrapBackendEnvelope(
  envelope: unknown,
  httpStatus: number,
  endpoint: string,
): BackendEnvelope<unknown> & Record<string, unknown> {
  if (!isEnvelopeObject(envelope)) {
    return {
      code: httpStatus,
      data: envelope,
    };
  }
  assertBackendEnvelopeMetadata(envelope, endpoint);
  const backendEnvelope = {
    ...envelope,
    code: envelope.code ?? httpStatus,
    data: envelope.data,
  };
  if (backendEnvelope.code !== 200) {
    throw new ApiError(
      backendEnvelope.code,
      backendEnvelope.message ?? 'Request failed, please try again later',
      backendEnvelope,
    );
  }
  return backendEnvelope;
}

function assertBackendEnvelopeMetadata(
  envelope: Record<string, unknown>,
  endpoint: string,
): asserts envelope is BackendEnvelopeObject {
  for (const field of ['code', 'total', 'type'] as const) {
    const value = envelope[field];
    if (value === undefined || (typeof value === 'number' && Number.isFinite(value))) continue;
    throw new ApiContractError(
      endpoint,
      envelope,
      new TypeError(`Backend envelope field "${field}" must be a finite number`),
    );
  }
  const message = envelope.message;
  if (message !== undefined && typeof message !== 'string') {
    throw new ApiContractError(
      endpoint,
      envelope,
      new TypeError('Backend envelope field "message" must be a string'),
    );
  }
}

function isEnvelopeObject(envelope: unknown): envelope is Record<string, unknown> {
  if (envelope === null || typeof envelope !== 'object' || Array.isArray(envelope)) return false;
  if (envelope instanceof ArrayBuffer) return false;
  if (ArrayBuffer.isView(envelope)) return false;
  if (envelope instanceof Blob) return false;
  return true;
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

function isPostRequest(method: string | undefined): boolean {
  return (method ?? 'GET').toUpperCase() === 'POST';
}

function shouldFormEncode(data: unknown): data is Record<string, unknown> {
  if (!data || typeof data !== 'object') return false;
  if (data instanceof URLSearchParams) return false;
  if (data instanceof FormData) return false;
  if (data instanceof Blob) return false;
  if (data instanceof ArrayBuffer) return false;
  return true;
}

function firstValidationError(errors: Record<string, string[]> | undefined): string | undefined {
  if (!errors) return undefined;
  return Object.values(errors)[0]?.[0];
}

function serializeForm(data: unknown, nullFormValue: 'omit' | 'empty'): string {
  if (!data || typeof data !== 'object' || Array.isArray(data)) return '';
  const parts: string[] = [];
  const target = {
    append(key: string, value: unknown) {
      parts.push(`${key}=${encodeURIComponent(String(value))}`);
    },
  };
  const source = nullFormValue === 'empty' ? replaceNullFormValues(data, new WeakMap()) : data;
  axios.toFormData(source as object, target, {
    indexes: true,
    maxDepth: 20,
  });
  return parts.join('&');
}

function replaceNullFormValues(value: unknown, seen: WeakMap<object, unknown>): unknown {
  if (value === null) return '';
  if (value === undefined || typeof value !== 'object') return value;
  if (seen.has(value)) return seen.get(value);

  if (Array.isArray(value)) {
    const copy: unknown[] = [];
    seen.set(value, copy);
    for (const item of value) copy.push(replaceNullFormValues(item, seen));
    return copy;
  }

  if (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) {
    return value;
  }
  const copy: Record<string, unknown> = {};
  seen.set(value, copy);
  for (const [key, child] of Object.entries(value)) {
    copy[key] = replaceNullFormValues(child, seen);
  }
  return copy;
}
