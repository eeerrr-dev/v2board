import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
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
}

export interface BackendEnvelope<T> {
  data: T;
  total?: number;
  type?: number;
  message?: string;
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
    timeout: 30_000,
    withCredentials: true,
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
      const query = serializeForm(config.params);
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
      config.data = serializeForm(config.data);
    }
    return config;
  });

  instance.interceptors.response.use(
    (response) => response,
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
      return response.data.data;
    },
    requestEnvelope: async <T,>(config: AxiosRequestConfig) => {
      const response = await instance.request<BackendEnvelope<T>>(config);
      return response.data;
    },
    resolveAdminPath: (path) => {
      const securePath = options.adminSecurePath?.();
      if (!securePath) return path;
      return `/${securePath}${path}`;
    },
  };
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

function serializeForm(data: unknown): string {
  if (!data || typeof data !== 'object' || Array.isArray(data)) return '';
  const parts: string[] = [];
  for (const key of Object.keys(data)) {
    appendFormValue(key, (data as Record<string, unknown>)[key], parts);
  }
  return parts.join('&');
}

function appendFormValue(key: string, value: unknown, parts: string[]): void {
  if (value === undefined) return;
  if (value !== null && typeof value === 'object') {
    if (Array.isArray(value)) {
      value.forEach((item, index) => appendFormValue(`${key}[${index}]`, item, parts));
      return;
    }
    for (const childKey of Object.keys(value)) {
      appendFormValue(`${key}[${childKey}]`, (value as Record<string, unknown>)[childKey], parts);
    }
    return;
  }
  parts.push(`${key}=${encodeURIComponent(value === null ? '' : String(value))}`);
}
