import { adminFilterArraySchema, type AdminFilter } from '@v2board/api-client';

/**
 * Consumes a one-shot cross-page filter. Persisted browser state is an
 * untrusted boundary: obsolete or malformed values are discarded instead of
 * being cast into the current request contract.
 */
export function takeStoredAdminFilters(key: string): AdminFilter[] {
  if (typeof window === 'undefined') return [];

  const stored = window.sessionStorage.getItem(key);
  if (!stored) return [];
  window.sessionStorage.removeItem(key);

  try {
    const parsed: unknown = JSON.parse(stored);
    const result = adminFilterArraySchema.safeParse(parsed);
    return result.success ? result.data : [];
  } catch {
    return [];
  }
}
