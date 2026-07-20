import type { CheckLoginResult } from '@v2board/types';

/**
 * §6.12 granular admin RBAC (docs/api-dialect.md §6.12): the fixed permission
 * registry mirrored from the backend. `is_admin` bypasses it entirely; staff
 * sessions carry `{family}:read|write` grants (`write` implies `read`).
 */
export const ADMIN_PERMISSION_FAMILIES = [
  'config',
  'system',
  'servers',
  'plans',
  'orders',
  'payments',
  'coupons',
  'gift_cards',
  'users',
  'tickets',
  'notices',
  'knowledge',
  'stats',
] as const;

export type AdminPermissionFamily = (typeof ADMIN_PERMISSION_FAMILIES)[number];

/**
 * SPA route → §6.12 family, by the admin API family each page consumes.
 * Declaration order doubles as the staff landing preference
 * (`firstAllowedRoute`).
 */
const ROUTE_FAMILIES: readonly (readonly [string, AdminPermissionFamily])[] = [
  ['/dashboard', 'stats'],
  ['/config/system', 'config'],
  ['/config/payment', 'payments'],
  ['/server/manage', 'servers'],
  ['/server/group', 'servers'],
  ['/server/route', 'servers'],
  ['/plan', 'plans'],
  ['/order', 'orders'],
  ['/coupon', 'coupons'],
  ['/giftcard', 'gift_cards'],
  ['/user', 'users'],
  ['/notice', 'notices'],
  ['/ticket', 'tickets'],
  ['/knowledge', 'knowledge'],
  ['/queue', 'system'],
  ['/audit', 'system'],
];

function grants(session: CheckLoginResult): readonly string[] {
  return session.admin_permissions ?? [];
}

/** Whether this session may enter the admin SPA at all: full admins always,
 * staff once the operator granted at least one family. */
export function canEnterAdminNamespace(session: CheckLoginResult): boolean {
  if (session.is_admin) return true;
  return session.is_staff === true && grants(session).length > 0;
}

/** Read access to one registry family (`write` implies `read`). */
function sessionAllowsFamily(session: CheckLoginResult, family: AdminPermissionFamily): boolean {
  if (session.is_admin) return true;
  if (session.is_staff !== true) return false;
  return grants(session).includes(`${family}:read`) || grants(session).includes(`${family}:write`);
}

function routeFamily(pathname: string): AdminPermissionFamily | undefined {
  for (const [route, family] of ROUTE_FAMILIES) {
    if (pathname === route || pathname.startsWith(`${route}/`)) return family;
  }
  return undefined;
}

/** Whether the session may open an admin SPA route (unmapped paths — login,
 * `/` — are outside RBAC and stay allowed). */
export function sessionAllowsRoute(session: CheckLoginResult, pathname: string): boolean {
  if (session.is_admin) return true;
  const family = routeFamily(pathname);
  return family === undefined ? true : sessionAllowsFamily(session, family);
}

/** The staff landing route: the first nav destination its grants can read.
 * Falls back to `/dashboard` (admins, or a session with no readable route). */
export function firstAllowedRoute(session: CheckLoginResult): string {
  if (session.is_admin) return '/dashboard';
  for (const [route, family] of ROUTE_FAMILIES) {
    if (sessionAllowsFamily(session, family)) return route;
  }
  return '/dashboard';
}
