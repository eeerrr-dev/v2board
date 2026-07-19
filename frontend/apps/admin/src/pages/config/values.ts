import { stripBasePath } from '@v2board/config';
import { getAdminBasename } from '@/lib/runtime-config';
import type { ConfigFieldValue } from './schema';

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

export function configValuesEqual(left: ConfigFieldValue, right: ConfigFieldValue) {
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) && toText(left) === toText(right);
  }
  return toText(left) === toText(right);
}

// History routing (docs/api-dialect.md §10.1): a saved secure_path moves the
// whole admin base, so the current app-relative route is re-rooted under the
// new `/{admin_path}` prefix via a full-page replace.
export function adminSecurePathLocation(securePath: string, currentRoutePath: string) {
  const normalizedPath = securePath.trim().replace(/^\/+|\/+$/g, '');
  if (!normalizedPath) throw new Error('后台路径不能为空');
  const route = currentRoutePath.startsWith('/') ? currentRoutePath : '/config/system';
  return `/${normalizedPath}${route === '/' ? '/config/system' : route}`;
}

export function replaceAdminSecurePath(securePath: string) {
  const { pathname, search } = window.location;
  const currentRoutePath = `${stripBasePath(pathname, getAdminBasename())}${search}`;
  window.location.replace(adminSecurePathLocation(securePath, currentRoutePath));
}

/**
 * §4.1: integer settings travel as JSON numbers. An empty or non-numeric
 * input coerces to `null`, the §4.4 clear-to-built-in-default signal.
 */
export function parseBackendInteger(value: string): number | null {
  const parsed = parseInt(value, 10);
  return Number.isNaN(parsed) ? null : parsed;
}

/** §4.1 rate settings (distribution levels, trial hours) are JSON numbers. */
export function parseBackendNumber(value: string): number | null {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
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
  return Boolean(parseInt(toText(value)));
}
