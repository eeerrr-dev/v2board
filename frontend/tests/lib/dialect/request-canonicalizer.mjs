// Internal-dialect request canonicalizer (docs/api-dialect.md §13.2).
//
// Decodes a captured per-world request into one canonical
// `{routeId, params, body}` object so Tier-1 payload comparison happens on
// canonical semantics instead of wire bytes:
//
// - oracle world: form-urlencoded/bracket-array bodies, `current`/`pageSize`
//   pagination and `filter[i][…]` bracket clauses;
// - source world: JSON bodies, `page`/`per_page` pagination and the `filter`
//   JSON query param (§7).
//
// Both worlds run the same value canonicalization (§4.1): 0/1 flags on the
// §4.1 boolean inventory become booleans, bracket arrays become arrays,
// numeric strings become numbers, RFC 3339 timestamps become epoch seconds
// (§4.5), and the legacy filter `{key, condition, value}` clause folds onto
// the modern `{field, op, value}` vocabulary (`like` values stay raw strings
// — §7.1/§13.2). Route-specific §9 named-object folds land with their owning
// family waves.

import { matchRoute } from './route-map.mjs';

/** §4.1 — the 0/1 request/response flags that become JSON booleans. */
export const BOOLEAN_FLAG_FIELDS = Object.freeze(
  new Set([
    'banned',
    'show',
    'renew',
    'enable',
    'is_admin',
    'is_staff',
    'is_login',
    'auto_renewal',
    'remind_expire',
    'remind_traffic',
    'is_online',
    'allow_insecure',
    'insecure',
    'disable_sni',
    'zero_rtt_handshake',
    'current',
    'is_email_verify',
    'is_invite_force',
    'is_recaptcha',
    'is_telegram',
    'withdraw_close',
    'commission_distribution_enable',
    'is_forget',
  ]),
);

/** §4.1 — legacy field spellings folded onto their modern names. */
export const FIELD_RENAMES = Object.freeze({ isforget: 'is_forget' });

/** §8 — `current`/`pageSize`/`page_size` fold onto `page`/`per_page`. */
const PAGINATION_RENAMES = Object.freeze({
  current: 'page',
  pageSize: 'per_page',
  page_size: 'per_page',
});

/** §7.1 — legacy filter `condition` tokens fold onto the modern op set. */
export const FILTER_CONDITION_OPS = Object.freeze({
  '=': 'eq',
  is: 'eq',
  '!=': 'neq',
  '<>': 'neq',
  not: 'neq',
  like: 'like',
  模糊: 'like',
  '>': 'gt',
  '>=': 'gte',
  '<': 'lt',
  '<=': 'lte',
});

const RFC3339_PATTERN =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;

/**
 * Canonicalize one captured request. `url` may be absolute or a
 * pathname+search string; `postData` is the raw request body (JSON or
 * form-urlencoded). Returns `{routeId, params, body}` (§13.2).
 */
export function canonicalizeRequest(world, { method, url, postData, securePath }) {
  const requestUrl = new URL(url, 'http://canonical.invalid');
  const body = decodeRequestBody(postData);
  const match = matchRoute(world, {
    method,
    pathname: requestUrl.pathname,
    searchParams: requestUrl.searchParams,
    body,
    securePath,
  });
  // Path parameters canonicalize like query values so a modern path segment
  // (`/user/knowledge/{id}`, W3) equals its legacy `?id=` query spelling.
  const pathParams = {};
  for (const [name, value] of Object.entries(match?.params ?? {})) {
    pathParams[name] = canonicalizeValue(value, name);
  }
  const params = {
    ...pathParams,
    ...canonicalizeQueryParams(requestUrl.searchParams),
  };
  return foldRouteRequest({
    routeId: match?.id ?? null,
    params,
    body: body === null ? null : canonicalizeValue(body, null),
  });
}

/**
 * §13.2 route-specific folds, landed with each owning family wave. W4 (§5.5):
 * the canonical commerce request is the modern shape — the legacy order-save
 * body folds onto the §9.2 create-order union (the deposit sentinel
 * `plan_id: 0` + `period: "deposit"` dies; an empty `coupon_code` is omitted
 * per the §5.5 empty-coupon rule), and the legacy body-carried `trade_no` /
 * `method` selection folds onto path-identity `trade_no` + `{method_id}`.
 */
function foldRouteRequest(request) {
  const fold = ROUTE_REQUEST_FOLDS[request.routeId];
  return fold ? fold(request) : request;
}

const ROUTE_REQUEST_FOLDS = Object.freeze({
  'user.orders.create': (request) => {
    const body = request.body;
    if (!isPlainObject(body) || body.kind !== undefined) return request;
    if (body.period === 'deposit') {
      return { ...request, body: { kind: 'deposit', deposit_amount: body.deposit_amount } };
    }
    const { plan_id, period, coupon_code } = body;
    return {
      ...request,
      body: {
        kind: 'plan',
        plan_id,
        period,
        // §5.5: the legacy empty-string coupon spelling folds to omission.
        ...(coupon_code ? { coupon_code } : {}),
      },
    };
  },
  'user.orders.cancel': foldBodyTradeNoIntoParams,
  'user.orders.checkout': foldBodyCheckoutSelection,
  'user.orders.stripe-intent': foldBodyCheckoutSelection,
  // W5 (§9.4): the legacy body-carried session_id folds onto the modern
  // DELETE /user/sessions/{session_id} path identity.
  'user.sessions.delete': foldBodySessionIdIntoParams,
  // W8 (§5.7): the legacy body-carried ticket id folds onto the modern
  // /user/tickets/{id}/replies + /user/tickets/{id}/close path identity.
  'user.tickets.replies.create': foldBodyIdIntoParams,
  'user.tickets.close': foldBodyIdIntoParams,
});

function foldBodyIdIntoParams(request) {
  const body = request.body;
  if (!isPlainObject(body) || body.id === undefined) return request;
  const { id, ...rest } = body;
  return {
    ...request,
    params: { id, ...request.params },
    body: Object.keys(rest).length === 0 ? null : rest,
  };
}

function foldBodySessionIdIntoParams(request) {
  const body = request.body;
  if (!isPlainObject(body) || body.session_id === undefined) return request;
  const { session_id, ...rest } = body;
  return {
    ...request,
    params: { session_id, ...request.params },
    body: Object.keys(rest).length === 0 ? null : rest,
  };
}

function foldBodyTradeNoIntoParams(request) {
  const body = request.body;
  if (!isPlainObject(body) || body.trade_no === undefined) return request;
  const { trade_no, ...rest } = body;
  return {
    ...request,
    params: { trade_no, ...request.params },
    body: Object.keys(rest).length === 0 ? null : rest,
  };
}

function foldBodyCheckoutSelection(request) {
  const lifted = foldBodyTradeNoIntoParams(request);
  const body = lifted.body;
  if (!isPlainObject(body) || body.method === undefined) return lifted;
  const { method, ...rest } = body;
  return { ...lifted, body: { method_id: method, ...rest } };
}

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

/** Decode a JSON or form-urlencoded body into a plain object (or null). */
export function decodeRequestBody(postData) {
  if (postData === undefined || postData === null || postData === '') return null;
  const raw = String(postData);
  try {
    return JSON.parse(raw);
  } catch {
    return foldBracketEntries([...new URLSearchParams(raw).entries()]);
  }
}

/** Canonicalize query params: pagination renames, filters, bracket arrays. */
export function canonicalizeQueryParams(searchParams) {
  const entries = [];
  const bracketEntries = [];
  for (const [key, value] of searchParams?.entries() ?? []) {
    if (key.includes('[')) {
      bracketEntries.push([key, value]);
    } else {
      entries.push([key, value]);
    }
  }

  const params = {};
  for (const [key, value] of entries) {
    const name = PAGINATION_RENAMES[key] ?? FIELD_RENAMES[key] ?? key;
    if (name === 'filter') {
      params.filter = canonicalizeFilterClauses(parseJsonFilter(value));
      continue;
    }
    const canonical = canonicalizeValue(value, name);
    if (name in params) {
      params[name] = [...(Array.isArray(params[name]) ? params[name] : [params[name]]), canonical];
    } else {
      params[name] = canonical;
    }
  }

  const folded = foldBracketEntries(bracketEntries);
  for (const [key, value] of Object.entries(folded)) {
    if (key === 'filter') {
      params.filter = canonicalizeFilterClauses(value);
    } else {
      params[FIELD_RENAMES[key] ?? key] = canonicalizeValue(value, key);
    }
  }

  return params;
}

/**
 * Fold both filter dialects to one clause list: the legacy bracket
 * `{key, condition, value}` clause and the modern `{field, op, value}`
 * clause (§7.1) both canonicalize to `{field, op, value}`.
 */
export function canonicalizeFilterClauses(clauses) {
  if (!Array.isArray(clauses)) return clauses;
  return clauses.map((clause) => {
    if (!clause || typeof clause !== 'object') return clause;
    const field = clause.field ?? clause.key;
    const op = clause.op ?? FILTER_CONDITION_OPS[clause.condition] ?? clause.condition;
    let value = clause.value;
    // §7.1: the legacy `'null'` string sentinel dies; null is null.
    if (value === 'null' && (op === 'eq' || op === 'neq')) value = null;
    // §7.1/§13.2: `like` compares on the raw string — no coercion.
    if (op !== 'like') value = canonicalizeValue(value, field);
    return { field, op, value };
  });
}

function parseJsonFilter(value) {
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

/**
 * Canonicalize one decoded value (deeply). `fieldName` drives the §4.1
 * boolean-flag folding; scalar strings fold to numbers/booleans/epoch
 * seconds where the canonical type is unambiguous.
 */
export function canonicalizeValue(value, fieldName) {
  if (Array.isArray(value)) {
    return value.map((item) => canonicalizeValue(item, fieldName));
  }
  if (value && typeof value === 'object') {
    const result = {};
    for (const [key, child] of Object.entries(value)) {
      const name = FIELD_RENAMES[key] ?? key;
      result[name] = canonicalizeValue(child, name);
    }
    return result;
  }
  return canonicalizeScalar(value, fieldName);
}

function canonicalizeScalar(value, fieldName) {
  const isFlag = fieldName !== null && BOOLEAN_FLAG_FIELDS.has(fieldName);
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') {
    if (isFlag && (value === 0 || value === 1)) return value === 1;
    return value;
  }
  if (typeof value !== 'string') return value;
  if (isFlag && (value === '0' || value === '1')) return value === '1';
  if (value === 'true' && isFlag) return true;
  if (value === 'false' && isFlag) return false;
  if (RFC3339_PATTERN.test(value)) {
    const epochMs = Date.parse(value);
    if (Number.isFinite(epochMs)) return Math.floor(epochMs / 1000);
  }
  if (/^-?\d+(?:\.\d+)?$/.test(value)) {
    const numeric = Number(value);
    // Only fold when the numeric form round-trips exactly (keeps
    // identifier-like strings such as trade numbers and zero-padded codes).
    if (Number.isFinite(numeric) && String(numeric) === value) return numeric;
  }
  return value;
}

/**
 * Fold `name[0]`/`name[0][key]` bracket entries (the legacy form-array
 * dialect, §4.1) into real arrays/objects.
 */
export function foldBracketEntries(entries) {
  const result = {};
  for (const [rawKey, value] of entries) {
    const path = parseBracketPath(rawKey);
    if (!path) {
      result[rawKey] = value;
      continue;
    }
    let target = result;
    for (let index = 0; index < path.length; index += 1) {
      const segment = path[index];
      const isLeaf = index === path.length - 1;
      if (isLeaf) {
        target[segment] = value;
      } else {
        const nextSegment = path[index + 1];
        if (target[segment] === undefined) {
          target[segment] = typeof nextSegment === 'number' ? [] : {};
        }
        target = target[segment];
      }
    }
  }
  return result;
}

function parseBracketPath(key) {
  const match = /^([^[\]]+)((?:\[[^[\]]*\])+)$/.exec(key);
  if (!match) return null;
  const segments = [match[1]];
  for (const [, inner] of match[2].matchAll(/\[([^[\]]*)\]/g)) {
    segments.push(/^\d+$/.test(inner) ? Number(inner) : inner);
  }
  return segments;
}
