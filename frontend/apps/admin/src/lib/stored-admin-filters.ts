import { z } from 'zod';
import type { AdminFilter } from '@v2board/api-client';

// App-internal `{key, condition, value}` clause shape (the API layer
// translates it into the §7 DSL per list endpoint). Validated locally: the
// wire-facing api-client no longer ships a legacy filter schema (W14).
const storedAdminFilterSchema = z.array(
  z.object({
    key: z.string(),
    condition: z.string(),
    value: z.union([z.string(), z.number(), z.null()]),
  }),
);

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
    const result = storedAdminFilterSchema.safeParse(parsed);
    return result.success ? result.data : [];
  } catch {
    return [];
  }
}
