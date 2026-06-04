export interface LegacyHashRouteOptions {
  authenticatedFallback: string;
  canonicalPath?: string;
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

function normalizeCanonicalPath(path: string): string {
  const next = normalizePath(path);
  return next !== '/' && next.endsWith('/') ? next.slice(0, -1) : next;
}

function routePattern(route: string): RegExp {
  const escaped = route
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/:[^/]+/g, '[^/]+');
  return new RegExp(`^${escaped}$`);
}

function routePrefixLength(path: string, route: string): number | null {
  if (route === '/') return null;
  if (!route.includes(':')) {
    return path.startsWith(`${route}/`) ? route.length : null;
  }

  const escaped = route
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/:[^/]+/g, '[^/]+');
  const match = new RegExp(`^(${escaped})(?=/)`).exec(path);
  return match?.[1]?.length ?? null;
}

function isKnownRoute(path: string, routes: readonly string[]): boolean {
  return routes.some((route) => route === path || (route.includes(':') && routePattern(route).test(path)));
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

export function getNormalizedLegacyHashPath(
  routeSource: string,
  options: LegacyHashRouteOptions,
): string {
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
  } else if (hasAuth && options.publicRoutes.includes(path)) {
    path = options.authenticatedFallback;
  } else if (!hasAuth && !options.publicRoutes.includes(path)) {
    path = options.guestFallback;
  }

  return `${path}${query}`;
}

export function normalizeLegacyHashRoute(options: LegacyHashRouteOptions): void {
  if (typeof window === 'undefined') return;

  const rawHash = window.location.hash.startsWith('#') ? window.location.hash.slice(1) : '';
  const routeSource = rawHash || window.location.pathname;
  const nextHash = getNormalizedLegacyHashPath(routeSource, options);
  const nextPathname =
    options.canonicalPath === undefined
      ? window.location.pathname
      : normalizeCanonicalPath(options.canonicalPath);
  if (rawHash !== nextHash || window.location.pathname !== nextPathname) {
    window.history.replaceState(null, '', `${nextPathname}${window.location.search}#${nextHash}`);
  }
}

export function installLegacyHashRouteNormalizer(options: LegacyHashRouteOptions): () => void {
  if (typeof window === 'undefined') return () => undefined;

  let normalizing = false;
  const normalize = () => {
    if (normalizing) return;
    normalizing = true;
    try {
      normalizeLegacyHashRoute(options);
    } finally {
      normalizing = false;
    }
  };
  const originalPushState = window.history.pushState;
  const originalReplaceState = window.history.replaceState;
  const wrapStateWriter = (writer: History['pushState']): History['pushState'] =>
    function normalizedStateWriter(this: History, ...args: Parameters<History['pushState']>) {
      writer.apply(this, args);
      normalize();
    };
  const pushState = wrapStateWriter(originalPushState);
  const replaceState = wrapStateWriter(originalReplaceState);

  window.history.pushState = pushState;
  window.history.replaceState = replaceState;
  window.addEventListener('hashchange', normalize);
  window.addEventListener('popstate', normalize);
  return () => {
    if (window.history.pushState === pushState) window.history.pushState = originalPushState;
    if (window.history.replaceState === replaceState) {
      window.history.replaceState = originalReplaceState;
    }
    window.removeEventListener('hashchange', normalize);
    window.removeEventListener('popstate', normalize);
  };
}
