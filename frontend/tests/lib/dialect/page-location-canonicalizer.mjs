// SPA page-location canonicalizer (docs/api-dialect.md §13.4).
//
// After W1 the source world is history-routed (§10.1) while the oracle stays
// hash-routed forever, so raw `window.location` reads diverge on every
// location-asserting scenario. This adapter
//
//  (a) maps a canonical SPA route path to the per-world entry URL —
//      `/#/x?y` for a hash-routed world, `/x?y` for a path-routed world; and
//  (b) canonicalizes location reads (oracle `#/x?y`, source
//      `pathname+search`) into one canonical route object.
//
// Per Appendix A §W0 both worlds are still hash-routed; W1 flips the source
// world's dialect below in the same series that lands history routing.

export const ROUTING_DIALECTS = Object.freeze(['hash', 'path']);

/** The live per-world SPA routing dialect. W1 flips `source` to 'path'. */
export const worldRoutingDialect = Object.freeze({
  oracle: 'hash',
  source: 'hash',
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
 * the path itself for a path-routed world.
 */
export function entryUrlFor(routePath, world) {
  return entryUrlForDialect(routePath, routingDialectFor(world));
}

export function entryUrlForDialect(routePath, dialect) {
  assertDialect(dialect);
  const canonical = normalizeRoutePath(routePath);
  if (dialect === 'path') return canonical;
  return `/#${canonical}`;
}

/**
 * Canonicalize a location read (§13.4b) into one canonical route object
 * `{path, query}`. Accepts a `window.location`-shaped object
 * (`{pathname, search, hash}`) or an href string.
 */
export function canonicalizeLocation(world, location) {
  return canonicalizeLocationForDialect(routingDialectFor(world), location);
}

export function canonicalizeLocationForDialect(dialect, location) {
  assertDialect(dialect);
  const { pathname, search, hash } = toLocationParts(location);
  if (dialect === 'hash') {
    return parseRoute(stripHashPrefix(hash));
  }
  return parseRoute(`${pathname}${search}`);
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
