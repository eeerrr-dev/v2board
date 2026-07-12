export interface HashRouteOptions {
  authenticatedFallback: string;
  authenticatedPublicFallbackRoutes?: readonly string[];
  guestFallback: string;
  publicRoutes: readonly string[];
  routes: readonly string[];
  authStorageKey?: string;
  nestedPrefixes?: readonly string[];
}

function normalizePath(path: string): string {
  const next = path.trim();
  if (!next || next === '#') return '/';
  return next.startsWith('/') ? next : `/${next}`;
}

function routePattern(route: string): RegExp {
  const escaped = route.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/:[^/]+/g, '[^/]+');
  return new RegExp(`^${escaped}$`);
}

function routePrefixLength(path: string, route: string): number | null {
  if (route === '/') return null;
  if (!route.includes(':')) return path.startsWith(`${route}/`) ? route.length : null;
  const escaped = route.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/:[^/]+/g, '[^/]+');
  return new RegExp(`^(${escaped})(?=/)`).exec(path)?.[1]?.length ?? null;
}

function isKnownRoute(path: string, routes: readonly string[]): boolean {
  return routes.some(
    (route) => route === path || (route.includes(':') && routePattern(route).test(path)),
  );
}

function recoverNestedPrefix(
  path: string,
  prefixes: readonly string[],
  routes: readonly string[],
): string | null {
  let current = path;
  while (true) {
    let length = 0;
    for (const prefix of prefixes) {
      const next = routePrefixLength(current, prefix);
      if (next !== null && next > length) length = next;
    }
    if (!length) return null;
    current = normalizePath(current.slice(length));
    if (isKnownRoute(current, routes)) return current;
  }
}

export function getNormalizedHashPath(routeSource: string, options: HashRouteOptions): string {
  const queryIndex = routeSource.indexOf('?');
  const rawPath = queryIndex >= 0 ? routeSource.slice(0, queryIndex) : routeSource;
  const query = queryIndex >= 0 ? routeSource.slice(queryIndex) : '';
  const hasAuth =
    typeof window !== 'undefined' &&
    Boolean(window.localStorage.getItem(options.authStorageKey ?? 'authorization'));
  const nestedPrefixes = options.nestedPrefixes ?? options.publicRoutes;
  const normalizedRawPath = normalizePath(rawPath);
  const recoveredNestedPath = recoverNestedPrefix(
    normalizedRawPath,
    nestedPrefixes,
    options.routes,
  );
  let path = recoveredNestedPath ?? normalizedRawPath;

  if (!isKnownRoute(path, options.routes)) {
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
