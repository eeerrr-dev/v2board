import { ApiContractError } from './client';
import { isSessionExpiredProblem } from './dialect';

export const INLINE_MUTATION_ERROR_META = {
  errorPresentation: 'inline',
} as const;

// Shared TanStack Query retry policy: transient transport failures (no status)
// and server errors are worth at most two retries, while deterministic
// outcomes — 4xx responses and response-contract violations — never are.
export function shouldRetryQuery(failureCount: number, error: unknown): boolean {
  if (error instanceof ApiContractError) return false;
  const status = getErrorPresentation(error).status;
  return failureCount < 2 && (status === undefined || status === 0 || status >= 500);
}

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
  // The 401 session_expired problem already performs credential teardown +
  // redirect in the API client — a toast during that navigation is duplicate
  // feedback and prone to leaking session details onto the login screen. 403
  // authorization verdicts (permission_denied / step_up_required) keep their
  // historical silence: the step-up dialog or the surface owns those.
  if (
    isSessionExpiredProblem(error) ||
    presentation.status === 403 ||
    meta?.errorPresentation === 'inline'
  ) {
    return false;
  }
  notify(localize(presentation.message));
  return true;
}

function getErrorStatus(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const candidate = error as { status?: unknown; response?: { status?: unknown } };
  if (typeof candidate.status === 'number') return candidate.status;
  return typeof candidate.response?.status === 'number' ? candidate.response.status : undefined;
}
