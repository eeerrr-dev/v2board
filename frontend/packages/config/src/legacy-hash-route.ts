export interface LegacyHashRouteOptions {
  authenticatedFallback: string;
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
  const escaped = route
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/:[^/]+/g, '[^/]+');
  return new RegExp(`^${escaped}$`);
}

function isKnownRoute(path: string, routes: readonly string[]): boolean {
  return routes.some((route) => route === path || (route.includes(':') && routePattern(route).test(path)));
}

function stripNestedPrefix(path: string, prefixes: readonly string[]): string {
  const prefix = prefixes.find((item) => item !== '/' && path.startsWith(`${item}/`));
  if (!prefix) return path;
  return normalizePath(path.slice(prefix.length));
}

export function normalizeLegacyHashRoute(options: LegacyHashRouteOptions): void {
  if (typeof window === 'undefined') return;

  const rawHash = window.location.hash.startsWith('#') ? window.location.hash.slice(1) : '';
  const routeSource = rawHash || window.location.pathname;
  const queryIndex = routeSource.indexOf('?');
  const rawPath = queryIndex >= 0 ? routeSource.slice(0, queryIndex) : routeSource;
  const query = queryIndex >= 0 ? routeSource.slice(queryIndex) : '';
  const hasAuth = Boolean(window.localStorage.getItem(options.authStorageKey ?? 'authorization'));
  const nestedPrefixes = options.nestedPrefixes ?? options.publicRoutes;
  let path = stripNestedPrefix(normalizePath(rawPath), nestedPrefixes);

  if (!isKnownRoute(path, options.routes)) {
    path = hasAuth ? options.authenticatedFallback : options.guestFallback;
  } else if (!hasAuth && !options.publicRoutes.includes(path)) {
    path = options.guestFallback;
  }

  const nextHash = `${path}${query}`;
  if (rawHash !== nextHash) {
    window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}#${nextHash}`);
  }
}

export function installLegacyHashRouteNormalizer(options: LegacyHashRouteOptions): () => void {
  if (typeof window === 'undefined') return () => undefined;

  const normalize = () => normalizeLegacyHashRoute(options);
  window.addEventListener('hashchange', normalize);
  return () => window.removeEventListener('hashchange', normalize);
}
