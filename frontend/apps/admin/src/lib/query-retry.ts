import { ApiContractError, getErrorPresentation } from '@v2board/api-client';

export function shouldRetryAdminQuery(failureCount: number, error: unknown): boolean {
  if (error instanceof ApiContractError) return false;
  const status = getErrorPresentation(error).status;
  return failureCount < 2 && (status === undefined || status === 0 || status >= 500);
}
