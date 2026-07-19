// History-routing guard (docs/api-dialect.md §10.1/§10.3). This module
// replaces the retired pre-router hash normalization (`hash-route.ts`): the
// auth-redirect-safety logic (public-route matching, auth-storage-key gate)
// is preserved because auth redirect outcomes are Tier-1, while the legacy
// `#/…` entry form is now handled once at boot by the
// `legacy_hash_redirect_enable` translator below.

export interface RouteGuardMatch {
  /** The pattern-matched prefix of the path (react-router's `pathnameBase`). */
  pathnameBase: string;
}

export interface RouteGuardOptions {
  authenticatedFallback: string;
  authenticatedPublicFallbackRoutes?: readonly string[];
  guestFallback: string;
  publicRoutes: readonly string[];
  routes: readonly string[];
  /**
   * The app's auth localStorage key (each app exports its own constant from
   * lib/auth). Required so this pre-render gate can never silently read a
   * different key than the app's session store.
   */
  authStorageKey: string;
  nestedPrefixes?: readonly string[];
  /**
   * Route-pattern matcher. Each app injects react-router's `matchPath`
   * (`(route, path, end) => matchPath({ path: route, end }, path)`) so this
   * route normalization can never disagree with the router's own matching
   * semantics; the package stays router-free. `end` mirrors matchPath's
   * flag: exact match versus prefix match.
   */
  matchRoute: (route: string, path: string, end: boolean) => RouteGuardMatch | null;
}

function normalizePath(path: string): string {
  const next = path.trim();
  if (!next || next === '#') return '/';
  return next.startsWith('/') ? next : `/${next}`;
}

function routePrefixLength(path: string, route: string, options: RouteGuardOptions): number | null {
  if (route === '/') return null;
  const base = options.matchRoute(route, path, false)?.pathnameBase;
  return base && path.startsWith(`${base}/`) ? base.length : null;
}

function isKnownRoute(path: string, options: RouteGuardOptions): boolean {
  return options.routes.some((route) => options.matchRoute(route, path, true) !== null);
}

function recoverNestedPrefix(path: string, options: RouteGuardOptions): string | null {
  const prefixes = options.nestedPrefixes ?? options.publicRoutes;
  let current = path;
  while (true) {
    let length = 0;
    for (const prefix of prefixes) {
      const next = routePrefixLength(current, prefix, options);
      if (next !== null && next > length) length = next;
    }
    if (!length) return null;
    current = normalizePath(current.slice(length));
    if (isKnownRoute(current, options)) return current;
  }
}

export function getNormalizedRoutePath(routeSource: string, options: RouteGuardOptions): string {
  const queryIndex = routeSource.indexOf('?');
  const rawPath = queryIndex >= 0 ? routeSource.slice(0, queryIndex) : routeSource;
  const query = queryIndex >= 0 ? routeSource.slice(queryIndex) : '';
  const hasAuth =
    typeof window !== 'undefined' && Boolean(window.localStorage.getItem(options.authStorageKey));
  const normalizedRawPath = normalizePath(rawPath);
  const recoveredNestedPath = recoverNestedPrefix(normalizedRawPath, options);
  let path = recoveredNestedPath ?? normalizedRawPath;

  if (!isKnownRoute(path, options)) {
    path = hasAuth ? options.authenticatedFallback : options.guestFallback;
  } else if (
    hasAuth &&
    (options.authenticatedPublicFallbackRoutes ?? options.publicRoutes).includes(path)
  ) {
    path = options.authenticatedFallback;
  } else if (!hasAuth && !options.publicRoutes.includes(path)) {
    path = options.guestFallback;
  }

  return `${path}${query}`;
}

/**
 * Strip an app basename (react-router `basename`, e.g. `/{admin_path}`) from a
 * history pathname, yielding the app-relative route path. Pathnames outside
 * the basename are returned unchanged — the router would never have matched
 * them, so guards treat them as unknown routes rather than throwing.
 */
export function stripBasePath(pathname: string, basename: string): string {
  if (basename === '' || basename === '/') return pathname;
  const base = basename.replace(/\/+$/, '');
  if (pathname === base) return '/';
  if (pathname.startsWith(`${base}/`)) return pathname.slice(base.length);
  return pathname;
}

export interface LegacyHashRedirectOptions {
  /** Runtime-config `legacy_hash_redirect_enable` (docs/api-dialect.md §10.3). */
  enabled: boolean;
  /** The app's history base — `/` for the user app, `/{admin_path}` for admin. */
  basename?: string;
}

/**
 * §10.3 — translate a legacy `#/x?y` hash into the history URL for
 * `history.replaceState`, resolved against the app basename. Returns null for
 * invalid/foreign hashes (anything not starting `#/`), which are left alone.
 */
export function legacyHashHistoryUrl(hash: string, basename = '/'): string | null {
  if (!hash.startsWith('#/')) return null;
  const pathAndQuery = hash.slice(1);
  const base = basename === '/' ? '' : basename.replace(/\/+$/, '');
  return `${base}${pathAndQuery}`;
}

/**
 * §10.3 boot translator. Runs before router creation in both apps. A hash
 * never reaches the server, so old `/#/x?y` URLs always arrive as `/` (user)
 * or `/{admin_path}` (admin); with the toggle ON the boot translates them to
 * history URLs via `history.replaceState`. OFF ignores the hash entirely and
 * the router boots on the server-delivered path.
 */
export function applyLegacyHashRedirect(options: LegacyHashRedirectOptions): boolean {
  if (!options.enabled || typeof window === 'undefined') return false;
  const target = legacyHashHistoryUrl(window.location.hash, options.basename ?? '/');
  if (target === null) return false;
  window.history.replaceState(null, '', target);
  return true;
}
