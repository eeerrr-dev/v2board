export interface LegacyHashRouteOptions {
  authenticatedFallback: string;
  canonicalPath?: string;
  guestFallback: string;
  publicRoutes: readonly string[];
  routes: readonly string[];
  authStorageKey?: string;
  nestedPrefixes?: readonly string[];
}

export interface LegacyWhiteScreenRecoveryConfig {
  delay?: number;
  storageKey?: string;
  now?: () => number;
  replace?: (url: string) => void;
}

export interface LegacyDevModuleRecoveryConfig {
  storageKey?: string;
  maxAttempts?: number;
  now?: () => number;
  replace?: (url: string) => void;
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
    window.history.replaceState(
      window.history.state,
      '',
      `${nextPathname}${window.location.search}#${nextHash}`,
    );
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
      const writtenHref = window.location.href;
      normalize();
      if (window.location.href !== writtenHref) {
        window.dispatchEvent(new PopStateEvent('popstate'));
      }
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

const NON_BLANK_ROOT_SELECTOR = [
  'a',
  'button',
  'input',
  'select',
  'textarea',
  'img',
  'svg',
  'canvas',
  'iframe',
  'video',
  '[role]',
  '[aria-label]',
  '.fa',
  '.si',
  '.ant-spin',
  '[class*="loading"]',
  '[class*="spinner"]',
].join(',');

function elementIsEmpty(element: HTMLElement | null): boolean {
  if (!element) return true;
  if (element.textContent?.trim()) return false;
  return !element.querySelector(NON_BLANK_ROOT_SELECTOR);
}

function appIsEmpty(root: HTMLElement | null): boolean {
  return elementIsEmpty(root);
}

function legacyMainIsEmpty(root: HTMLElement | null): boolean {
  const page = root?.querySelector<HTMLElement>('#page-container');
  const main = page?.querySelector<HTMLElement>('#main-container');
  if (!page || !main) return false;
  return elementIsEmpty(main);
}

function stableRecoveryKey(url: URL): string {
  const search = new URLSearchParams(url.search);
  search.delete('__v2board_recover');
  search.delete('__v2board_dev_recover');
  const query = search.toString();
  return `${url.pathname}${query ? `?${query}` : ''}${url.hash}`;
}

function getAuthStorageKey(options: LegacyHashRouteOptions): string {
  return options.authStorageKey ?? 'authorization';
}

function hasLegacyAuth(options: LegacyHashRouteOptions): boolean {
  return Boolean(window.localStorage.getItem(getAuthStorageKey(options)));
}

function getFallbackHash(options: LegacyHashRouteOptions, hasAuth = hasLegacyAuth(options)): string {
  return `#${hasAuth ? options.authenticatedFallback : options.guestFallback}`;
}

function normalizeEmptyRootUrl(current: URL, options: LegacyHashRouteOptions): boolean {
  const rawHash = current.hash.startsWith('#') ? current.hash.slice(1) : '';
  const routeSource = rawHash || current.pathname;
  const nextHash = `#${getNormalizedLegacyHashPath(routeSource, options)}`;
  const nextPathname =
    options.canonicalPath === undefined
      ? current.pathname
      : normalizeCanonicalPath(options.canonicalPath);

  if (current.pathname === nextPathname && current.hash === nextHash) return false;

  current.pathname = nextPathname;
  current.hash = nextHash;
  return true;
}

export function installLegacyWhiteScreenRecovery(
  options: LegacyHashRouteOptions,
  config: LegacyWhiteScreenRecoveryConfig = {},
): () => void {
  if (typeof window === 'undefined' || typeof document === 'undefined') return () => undefined;

  const delay = config.delay ?? 1200;
  const storageKey = config.storageKey ?? 'v2board:white-screen-recovery';
  const now = config.now ?? (() => Date.now());
  const replace = config.replace ?? ((url: string) => window.location.replace(url));
  let timer: number | undefined;
  let emptyShellKey = '';
  let emptyShellChecks = 0;

  const recoverIfEmpty = () => {
    const root = document.getElementById('root');
    const current = new URL(window.location.href);
    const key = `${storageKey}:${stableRecoveryKey(current)}`;
    const rootIsEmpty = appIsEmpty(root);
    const shellMainIsEmpty = !rootIsEmpty && legacyMainIsEmpty(root);

    if (!rootIsEmpty && !shellMainIsEmpty) {
      window.sessionStorage.removeItem(key);
      emptyShellKey = '';
      emptyShellChecks = 0;
      return;
    }

    if (shellMainIsEmpty) {
      const currentShellKey = stableRecoveryKey(current);
      if (emptyShellKey !== currentShellKey) {
        emptyShellKey = currentShellKey;
        emptyShellChecks = 0;
      }
      emptyShellChecks += 1;
      if (emptyShellChecks < 2) {
        timer = window.setTimeout(recoverIfEmpty, delay);
        return;
      }
    }

    const attempts = Number(window.sessionStorage.getItem(key) ?? '0');
    const hasAuth = hasLegacyAuth(options);
    const fallbackHash = getFallbackHash(options, hasAuth);
    current.searchParams.set('__v2board_recover', String(now()));

    if (normalizeEmptyRootUrl(current, options)) {
      window.sessionStorage.setItem(key, String(attempts + 1));
      replace(current.toString());
      return;
    }

    if (attempts >= 2) {
      if (current.hash === fallbackHash) {
        if (!hasAuth || fallbackHash !== `#${options.authenticatedFallback}`) return;
        window.localStorage.removeItem(getAuthStorageKey(options));
        window.sessionStorage.setItem(key, String(attempts + 1));
        current.hash = `#${options.guestFallback}`;
        replace(current.toString());
        return;
      }
      window.sessionStorage.setItem(key, String(attempts + 1));
      current.hash = fallbackHash;
      replace(current.toString());
      return;
    }

    window.sessionStorage.setItem(key, String(attempts + 1));

    if (attempts > 0) {
      current.hash = fallbackHash;
    }

    replace(current.toString());
  };

  const schedule = () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(recoverIfEmpty, delay);
  };

  const root = document.getElementById('root');
  const observer =
    root && typeof MutationObserver !== 'undefined'
      ? new MutationObserver(schedule)
      : undefined;

  observer?.observe(root as HTMLElement, { childList: true, subtree: true });
  window.addEventListener('hashchange', schedule);
  window.addEventListener('popstate', schedule);
  window.addEventListener('error', schedule);
  window.addEventListener('unhandledrejection', schedule);
  schedule();

  return () => {
    if (timer !== undefined) window.clearTimeout(timer);
    observer?.disconnect();
    window.removeEventListener('hashchange', schedule);
    window.removeEventListener('popstate', schedule);
    window.removeEventListener('error', schedule);
    window.removeEventListener('unhandledrejection', schedule);
  };
}

function errorText(value: unknown, seen = new Set<unknown>()): string {
  if (value === null || value === undefined) return '';
  if (value instanceof Error) return `${value.message}\n${value.stack ?? ''}`;
  if (typeof value !== 'object') return String(value);
  if (seen.has(value)) return '';
  seen.add(value);

  const record = value as Record<string, unknown>;
  const fields = [
    'message',
    'stack',
    'name',
    'code',
    'url',
    'id',
    'file',
    'filename',
    'plugin',
    'frame',
    'cause',
    'error',
    'err',
    'reason',
    'detail',
    'data',
    'payload',
  ];

  return fields.map((field) => errorText(record[field], seen)).join('\n');
}

function isStaleViteModuleText(value: unknown): boolean {
  const text = errorText(value);
  if (!text) return false;
  const lower = text.toLowerCase();
  return (
    text.includes('Outdated Optimize Dep') ||
    text.includes('504 (Outdated Optimize Dep)') ||
    text.includes('Failed to fetch dynamically imported module') ||
    lower.includes('error loading dynamically imported module') ||
    text.includes('Importing a module script failed') ||
    text.includes('Failed to load module script') ||
    lower.includes('failed to import module script') ||
    lower.includes('chunkloaderror') ||
    lower.includes('loading chunk') ||
    (lower.includes('requested module') &&
      (lower.includes('does not provide an export') ||
        lower.includes("doesn't provide an export"))) ||
    text.includes('/node_modules/.vite/deps/') ||
    (text.includes('/node_modules/.vite/') &&
      (lower.includes('does not provide an export named') ||
        lower.includes('mime type') ||
        lower.includes('module script') ||
        lower.includes('net::err_aborted')))
  );
}

function isStaleViteModuleEvent(event: Event): boolean {
  const maybeError = event as ErrorEvent & {
    detail?: unknown;
    payload?: unknown;
    reason?: unknown;
  };
  return isStaleViteModuleText(
    [
      errorText(maybeError.message),
      errorText(maybeError.filename),
      errorText(maybeError.error),
      errorText(maybeError.reason),
      errorText(maybeError.detail),
      errorText(maybeError.payload),
    ].join('\n'),
  );
}

export function installLegacyDevModuleRecovery(
  config: LegacyDevModuleRecoveryConfig = {},
): () => void {
  if (typeof window === 'undefined') return () => undefined;

  const storageKey = config.storageKey ?? 'v2board:dev-module-recovery';
  const maxAttempts = config.maxAttempts ?? 3;
  const now = config.now ?? (() => Date.now());
  const replace = config.replace ?? ((url: string) => window.location.replace(url));

  const recover = (event: Event) => {
    if (!isStaleViteModuleEvent(event)) return;
    event.preventDefault();

    const current = new URL(window.location.href);
    const key = `${storageKey}:${stableRecoveryKey(current)}`;
    const attempts = Number(window.sessionStorage.getItem(key) ?? '0');
    if (attempts >= maxAttempts) return;

    window.sessionStorage.setItem(key, String(attempts + 1));
    current.searchParams.set('__v2board_dev_recover', String(now()));
    replace(current.toString());
  };

  window.addEventListener('vite:preloadError', recover);
  window.addEventListener('vite:error', recover);
  window.addEventListener('error', recover, true);
  window.addEventListener('unhandledrejection', recover);

  return () => {
    window.removeEventListener('vite:preloadError', recover);
    window.removeEventListener('vite:error', recover);
    window.removeEventListener('error', recover, true);
    window.removeEventListener('unhandledrejection', recover);
  };
}
