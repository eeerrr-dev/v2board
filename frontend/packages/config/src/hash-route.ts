export interface HashRouteMatch {
  /** The pattern-matched prefix of the path (react-router's `pathnameBase`). */
  pathnameBase: string;
}

export interface HashRouteOptions {
  authenticatedFallback: string;
  authenticatedPublicFallbackRoutes?: readonly string[];
  guestFallback: string;
  publicRoutes: readonly string[];
  routes: readonly string[];
  /**
   * The app's auth localStorage key (each app exports its own constant from
   * lib/auth). Required so this pre-router gate can never silently read a
   * different key than the app's session store.
   */
  authStorageKey: string;
  nestedPrefixes?: readonly string[];
  /**
   * Route-pattern matcher. Each app injects react-router's `matchPath`
   * (`(route, path, end) => matchPath({ path: route, end }, path)`) so this
   * pre-router hash normalization can never disagree with the router's own
   * matching semantics; the package stays router-free. `end` mirrors
   * matchPath's flag: exact match versus prefix match.
   */
  matchRoute: (route: string, path: string, end: boolean) => HashRouteMatch | null;
}

function normalizePath(path: string): string {
  const next = path.trim();
  if (!next || next === '#') return '/';
  return next.startsWith('/') ? next : `/${next}`;
}

function routePrefixLength(path: string, route: string, options: HashRouteOptions): number | null {
  if (route === '/') return null;
  const base = options.matchRoute(route, path, false)?.pathnameBase;
  return base && path.startsWith(`${base}/`) ? base.length : null;
}

function isKnownRoute(path: string, options: HashRouteOptions): boolean {
  return options.routes.some((route) => options.matchRoute(route, path, true) !== null);
}

function recoverNestedPrefix(path: string, options: HashRouteOptions): string | null {
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

export function getNormalizedHashPath(routeSource: string, options: HashRouteOptions): string {
  const queryIndex = routeSource.indexOf('?');
  const rawPath = queryIndex >= 0 ? routeSource.slice(0, queryIndex) : routeSource;
  const query = queryIndex >= 0 ? routeSource.slice(queryIndex) : '';
  const hasAuth =
    typeof window !== 'undefined' &&
    Boolean(window.localStorage.getItem(options.authStorageKey));
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
