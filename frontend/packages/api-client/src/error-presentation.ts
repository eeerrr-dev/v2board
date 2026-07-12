export const INLINE_MUTATION_ERROR_META = {
  errorPresentation: 'inline',
} as const;

export type MutationErrorMeta = Readonly<Record<string, unknown>> | undefined;

export interface ErrorPresentation {
  message: string;
  status: number | undefined;
}

export function getErrorPresentation(error: unknown): ErrorPresentation {
  if (error instanceof Error) {
    return {
      message: error.message || 'Request failed, please try again later',
      status: getErrorStatus(error),
    };
  }
  if (typeof error === 'object' && error !== null) {
    const candidate = error as { message?: unknown };
    return {
      message:
        typeof candidate.message === 'string' && candidate.message
          ? candidate.message
          : 'Request failed, please try again later',
      status: getErrorStatus(error),
    };
  }
  return {
    message: typeof error === 'string' && error ? error : 'Request failed, please try again later',
    status: undefined,
  };
}

export function presentMutationError(
  error: unknown,
  meta: MutationErrorMeta,
  notify: (message: string) => void,
  localize: (message: string) => string = (message) => message,
): boolean {
  const presentation = getErrorPresentation(error);
  // 403 already performs credential teardown + redirect in the API client. A
  // toast during navigation is both duplicate feedback and prone to leaking
  // session details onto the login screen.
  if (presentation.status === 403 || meta?.errorPresentation === 'inline') return false;
  notify(localize(presentation.message));
  return true;
}

function getErrorStatus(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const candidate = error as { status?: unknown; response?: { status?: unknown } };
  if (typeof candidate.status === 'number') return candidate.status;
  return typeof candidate.response?.status === 'number' ? candidate.response.status : undefined;
}
