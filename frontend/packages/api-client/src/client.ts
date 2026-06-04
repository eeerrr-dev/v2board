import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
  type AxiosResponse,
} from 'axios';

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
    public readonly raw?: unknown,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export interface ApiErrorHook {
  (error: ApiError): void;
}

export interface ApiClientOptions {
  baseURL?: string;
  getAuthData?: () => string | null;
  getLocale?: () => string | null;
  onUnauthorized?: ApiErrorHook;
  onError?: ApiErrorHook;
  adminSecurePath?: () => string | null;
  nullFormValue?: 'omit' | 'empty';
}

export interface BackendEnvelope<T> {
  code?: number;
  data: T;
  total?: number;
  type?: number;
  buffer?: unknown;
  message?: string;
  msg?: string;
}

export interface ApiClient {
  axios: AxiosInstance;
  request: <T>(config: AxiosRequestConfig) => Promise<T>;
  requestEnvelope: <T>(config: AxiosRequestConfig) => Promise<BackendEnvelope<T>>;
  resolveAdminPath: (path: string) => string;
}

export function createApiClient(options: ApiClientOptions = {}): ApiClient {
  const instance = axios.create({
    baseURL: options.baseURL ?? '/api/v1',
    withCredentials: true,
    validateStatus: (status) => status === 200,
  });

  instance.interceptors.request.use((config) => {
    const token = options.getAuthData?.();
    const locale = options.getLocale?.();
    config.headers = config.headers ?? {};
    if (token) {
      config.headers.authorization = token;
    }
    if (locale) config.headers['Content-Language'] = locale;
    if (config.params) {
      const query = serializeForm(config.params, options.nullFormValue ?? 'omit');
      if (query) {
        const separator = config.url?.includes('?') ? '&' : '?';
        config.url = `${config.url}${separator}${query}`;
      }
      config.params = undefined;
    }
    if (isLegacyPost(config.method) && config.data === undefined) {
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
      const message =
        firstValidationError(error.response?.data?.errors) ??
        error.response?.data?.message ??
        error.message ??
        'Request failed, please try again later';
      const apiError = new ApiError(status, message, error.response?.data);
      if (status === 403) {
        options.onUnauthorized?.(apiError);
      } else {
        options.onError?.(apiError);
      }
      return Promise.reject(apiError);
    },
  );

  return {
    axios: instance,
    request: async <T,>(config: AxiosRequestConfig) => {
      const response = await instance.request<BackendEnvelope<T>>(config);
      return unwrapLegacyEnvelope(response.data, response.status, options).data;
    },
    requestEnvelope: async <T,>(config: AxiosRequestConfig) => {
      const response = await instance.request<BackendEnvelope<T>>(config);
      return unwrapLegacyEnvelope(response.data, response.status, options);
    },
    resolveAdminPath: (path) => {
      const securePath = options.adminSecurePath?.();
      if (!securePath) return path;
      return `/${securePath}${path}`;
    },
  };
}

function unwrapLegacyEnvelope<T>(
  envelope: BackendEnvelope<T>,
  httpStatus: number,
  options: ApiClientOptions,
): BackendEnvelope<T> {
  if (!isEnvelopeObject(envelope)) {
    return {
      code: httpStatus,
      data: envelope as T,
      buffer: envelope,
    };
  }
  const legacyEnvelope = { code: httpStatus, ...envelope };
  if (legacyEnvelope.code !== 200) {
    const apiError = new ApiError(
      legacyEnvelope.code,
      legacyEnvelope.message ?? legacyEnvelope.msg ?? 'Request failed, please try again later',
      legacyEnvelope,
    );
    if (legacyEnvelope.code === 403) options.onUnauthorized?.(apiError);
    throw apiError;
  }
  return legacyEnvelope;
}

function isEnvelopeObject<T>(envelope: BackendEnvelope<T>): envelope is BackendEnvelope<T> {
  if (envelope === null || typeof envelope !== 'object' || Array.isArray(envelope)) return false;
  if (envelope instanceof ArrayBuffer) return false;
  if (ArrayBuffer.isView(envelope)) return false;
  if (typeof Blob !== 'undefined' && envelope instanceof Blob) return false;
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

function getContentType(headers: unknown): string {
  const maybeHeaders = headers as
    | { get?: (name: string) => unknown; [key: string]: unknown }
    | undefined;
  const value =
    typeof maybeHeaders?.get === 'function'
      ? maybeHeaders.get('content-type')
      : maybeHeaders?.['content-type'] ?? maybeHeaders?.['Content-Type'];
  return typeof value === 'string' ? value : String(value ?? '');
}

function toArrayBuffer(data: unknown): ArrayBuffer | null {
  if (data instanceof ArrayBuffer) return data;
  if (!ArrayBuffer.isView(data)) return null;
  const view = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  return view.slice().buffer;
}

function isLegacyPost(method: string | undefined): boolean {
  return (method ?? 'GET').toUpperCase() === 'POST';
}

function shouldFormEncode(data: unknown): data is Record<string, unknown> {
  if (!data || typeof data !== 'object') return false;
  if (data instanceof URLSearchParams) return false;
  if (typeof FormData !== 'undefined' && data instanceof FormData) return false;
  if (typeof Blob !== 'undefined' && data instanceof Blob) return false;
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
  for (const key in data as Record<string, unknown>) {
    appendFormValue(key, (data as Record<string, unknown>)[key], parts, nullFormValue);
  }
  return parts.join('&');
}

function appendFormValue(
  key: string,
  value: unknown,
  parts: string[],
  nullFormValue: 'omit' | 'empty',
): void {
  if (value === undefined) return;
  if (value === null) {
    if (nullFormValue === 'empty') parts.push(`${key}=`);
    return;
  }
  if (value !== null && typeof value === 'object') {
    for (const childKey in value as Record<string, unknown>) {
      appendFormValue(
        `${key}[${childKey}]`,
        (value as Record<string, unknown>)[childKey],
        parts,
        nullFormValue,
      );
    }
    return;
  }
  parts.push(`${key}=${encodeURIComponent(String(value))}`);
}
