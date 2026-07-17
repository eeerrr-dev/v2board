// SPA page-location canonicalizer (docs/api-dialect.md §13.4).
//
// Since W1 the source world is history-routed (§10.1) while the oracle stays
// hash-routed forever, so raw `window.location` reads diverge on every
// location-asserting scenario. This adapter
//
//  (a) maps a canonical SPA route path to the per-world entry URL —
//      `/#/x?y` for a hash-routed world, `/x?y` for a path-routed world
//      (admin worlds prefix their `/{admin_path}` base); and
//  (b) canonicalizes location reads (oracle `#/x?y`, source
//      `pathname+search`) into one canonical route form.
//
// For in-page reads and navigations the harness installs the browser-side
// helpers below (installSpaLocationHelpers) so one run(page) stays
// world-agnostic: `window.__parityReadSpaRoute()` returns the canonical
// `/x?y` route string in both worlds and `window.__paritySpaNavigate('/x')`
// performs an in-place SPA navigation in either dialect.

export const ROUTING_DIALECTS = Object.freeze(['hash', 'path']);

/** The live per-world SPA routing dialect. W1 flipped `source` to 'path'. */
export const worldRoutingDialect = Object.freeze({
  oracle: 'hash',
  source: 'path',
});

export function routingDialectFor(world) {
  const dialect = worldRoutingDialect[world];
  if (!dialect) {
    throw new Error(`Unknown parity world "${world}" (expected oracle | source)`);
  }
  return dialect;
}

/**
 * Map a canonical SPA route path (`/order/T1?cashier=1`) to the per-world
 * entry URL path (§13.4a): `/#/order/T1?cashier=1` for a hash-routed world,
 * the path itself for a path-routed world. Admin worlds pass their
 * `/{admin_path}` mount as `basePath` (`/admin-path#/plan` vs
 * `/admin-path/plan`).
 */
export function entryUrlFor(routePath, world, basePath = '') {
  return entryUrlForDialect(routePath, routingDialectFor(world), basePath);
}

export function entryUrlForDialect(routePath, dialect, basePath = '') {
  assertDialect(dialect);
  const canonical = normalizeRoutePath(routePath);
  const base = normalizeBasePath(basePath);
  if (dialect === 'path') return `${base}${canonical}`;
  return base === '' ? `/#${canonical}` : `${base}#${canonical}`;
}

/**
 * Canonicalize a location read (§13.4b) into one canonical route object
 * `{path, query}`. Accepts a `window.location`-shaped object
 * (`{pathname, search, hash}`) or an href string. Path-dialect reads strip
 * the admin `basePath` mount when one is given.
 */
export function canonicalizeLocation(world, location, basePath = '') {
  return canonicalizeLocationForDialect(routingDialectFor(world), location, basePath);
}

export function canonicalizeLocationForDialect(dialect, location, basePath = '') {
  assertDialect(dialect);
  const { pathname, search, hash } = toLocationParts(location);
  if (dialect === 'hash') {
    return parseRoute(stripHashPrefix(hash));
  }
  return parseRoute(`${stripBasePath(pathname, normalizeBasePath(basePath))}${search}`);
}

/**
 * Browser-side bootstrap (serialized into the page via addInitScript).
 * Installs the two world-agnostic helpers every runner/state-reader uses:
 *
 *  - `window.__parityReadSpaRoute()` → the canonical `/x?y` route string;
 *  - `window.__paritySpaNavigate('/x?y')` → an in-place SPA navigation
 *    (hash assignment in the hash dialect; pushState + popstate in the path
 *    dialect so the history router processes the transition without a
 *    reload).
 */
export function spaLocationHelpersBootstrap({ dialect, basePath }) {
  window.__PARITY_SPA_DIALECT__ = dialect;
  window.__PARITY_SPA_BASE__ = basePath || '';
  window.__parityReadSpaRoute = function readSpaRoute() {
    if (window.__PARITY_SPA_DIALECT__ === 'hash') {
      const raw = String(window.location.hash || '');
      const stripped = raw.startsWith('#') ? raw.slice(1) : raw;
      return stripped || '/';
    }
    const base = window.__PARITY_SPA_BASE__ || '';
    let path = window.location.pathname || '/';
    if (base && (path === base || path.startsWith(`${base}/`))) {
      path = path.slice(base.length) || '/';
    }
    return `${path}${window.location.search || ''}`;
  };
  window.__paritySpaNavigate = function spaNavigate(routePath) {
    const raw = String(routePath ?? '/');
    const canonical = raw.startsWith('/') ? raw : `/${raw}`;
    if (window.__PARITY_SPA_DIALECT__ === 'hash') {
      window.location.hash = `#${canonical}`;
      return;
    }
    const base = window.__PARITY_SPA_BASE__ || '';
    window.history.pushState(null, '', `${base}${canonical}`);
    window.dispatchEvent(new PopStateEvent('popstate'));
  };
}

/** Install the browser-side helpers for one world before any navigation. */
export async function installSpaLocationHelpers(page, world, basePath = '') {
  await page.addInitScript(spaLocationHelpersBootstrap, {
    dialect: routingDialectFor(world),
    basePath: normalizeBasePath(basePath),
  });
}

function assertDialect(dialect) {
  if (!ROUTING_DIALECTS.includes(dialect)) {
    throw new Error(`Unknown routing dialect "${dialect}" (expected ${ROUTING_DIALECTS.join(' | ')})`);
  }
}

function toLocationParts(location) {
  if (typeof location === 'string') {
    const url = new URL(location, 'http://canonical.invalid');
    return { pathname: url.pathname, search: url.search, hash: url.hash };
  }
  return {
    pathname: location?.pathname ?? '/',
    search: location?.search ?? '',
    hash: location?.hash ?? '',
  };
}

function stripHashPrefix(hash) {
  const raw = String(hash ?? '');
  if (raw === '' || raw === '#') return '/';
  return raw.startsWith('#') ? raw.slice(1) : raw;
}

function stripBasePath(pathname, base) {
  if (!base) return pathname;
  if (pathname === base) return '/';
  if (pathname.startsWith(`${base}/`)) return pathname.slice(base.length);
  return pathname;
}

function parseRoute(routeWithQuery) {
  const normalized = normalizeRoutePath(routeWithQuery);
  const url = new URL(normalized, 'http://canonical.invalid');
  const query = {};
  for (const [key, value] of url.searchParams.entries()) {
    if (key in query) {
      query[key] = [...(Array.isArray(query[key]) ? query[key] : [query[key]]), value];
    } else {
      query[key] = value;
    }
  }
  return { path: url.pathname, query };
}

function normalizeRoutePath(routePath) {
  const raw = String(routePath ?? '/');
  return raw.startsWith('/') ? raw : `/${raw}`;
}

function normalizeBasePath(basePath) {
  const raw = String(basePath ?? '').trim();
  if (raw === '' || raw === '/') return '';
  const prefixed = raw.startsWith('/') ? raw : `/${raw}`;
  return prefixed.replace(/\/+$/, '');
}
