import { stripBasePath } from '@v2board/config';
import { getAdminBasename } from '@/lib/runtime-config';

// --- Backend-contract value coercions --------------------------------------

export function toText(value: unknown) {
  if (Array.isArray(value)) return value.join(',');
  return value == null ? '' : String(value);
}

export function splitComma(value: string) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

// History routing (docs/api-dialect.md §10.1): a saved secure_path moves the
// whole admin base, so the current app-relative route is re-rooted under the
// new `/{admin_path}` prefix via a full-page replace.
export function adminSecurePathLocation(securePath: string, currentRoutePath: string) {
  const normalizedPath = securePath.trim().replace(/^\/+|\/+$/g, '');
  // Dotted i18n key: the section form surfaces this through FieldError, which
  // resolves it via translateRuntimeMessage (module scope has no `t`).
  if (!normalizedPath) throw new Error('admin.config.secure_path_required');
  const route = currentRoutePath.startsWith('/') ? currentRoutePath : '/config/system';
  return `/${normalizedPath}${route === '/' ? '/config/system' : route}`;
}

export function replaceAdminSecurePath(securePath: string) {
  const { pathname, search } = window.location;
  const currentRoutePath = `${stripBasePath(pathname, getAdminBasename())}${search}`;
  window.location.replace(adminSecurePathLocation(securePath, currentRoutePath));
}

/**
 * §4.1: integer settings travel as JSON numbers. An empty input maps to the
 * §4.4 clear signal. Malformed or unsafe values are rejected in full instead
 * of being silently prefix-parsed or truncated.
 */
export function parseBackendInteger(value: string): number | null {
  const normalized = value.trim();
  if (normalized === '') return null;
  if (!/^[+-]?\d+$/.test(normalized)) {
    throw new TypeError('admin.config.integer_invalid');
  }
  const parsed = Number(normalized);
  if (!Number.isSafeInteger(parsed)) {
    throw new TypeError('admin.config.integer_invalid');
  }
  return parsed;
}

/** §4.1 decimal settings are finite JSON numbers parsed from the whole input. */
export function parseBackendNumber(value: string): number | null {
  const normalized = value.trim();
  if (normalized === '') return null;
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(normalized)) {
    throw new TypeError('admin.config.number_invalid');
  }
  const parsed = Number(normalized);
  if (!Number.isFinite(parsed)) {
    throw new TypeError('admin.config.number_invalid');
  }
  return parsed;
}

/** Maps a Radix Select option id to its §4.1 JSON-integer wire value. */
export function selectInteger(value: string): number {
  return Number(value);
}

/** Maps the legacy '0'/'1' order-event option ids to wire booleans. */
export function selectBoolean(value: string): boolean {
  return value === '1';
}

export function isBackendEnabled(value: unknown) {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return value === 1;
  return typeof value === 'string' && value.trim() === '1';
}
