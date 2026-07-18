// World-aware fixture emitters (docs/api-dialect.md §13.5).
//
// `fixture-data.mjs` stays the canonical data source; `api-fixtures.mjs`
// serializes every intercepted API response through this per-world seam.
// A fixture response is the legacy-envelope object produced by
// `apiFixtureResponse` — `{code, data, total?, type?, message?}` plus the
// optional `httpStatus`/`rawBody`/`contentType` transport overrides — and an
// emitter turns it into the wire `{status, contentType, body}`.
//
// Family waves taught the source emitter modern shapes route-by-route (bare
// objects / `{items,total}`, RFC 3339, booleans, real HTTP semantics with
// problem+json bodies) while the oracle emitter keeps legacy forever. A
// migrated fixture carries `dialect: 'v2'` (W2 flipped the §5.2 auth family;
// W3 the §5.1 public + §5.8 content family; W4 the §5.5 user commerce
// family; W5 the §5.3/§5.4 profile + subscription family; W6 the §5.4
// service-usage family; W7 the §5.6 invite & commission family; W8 the
// §5.7 user ticket family; W9 the §6.1 admin config & system family; W10
// the §6.3 admin content family; W11 the §6.2/§6.4 admin commerce family;
// W12 the §6.6 admin users family — the §7 filter/sort DSL flagship;
// W13 the §6.7 admin servers family; W14 the §6.5 admin tickets + §6.8
// stats + §6.9 staff mirror, closing the wave series). Every internal
// family now emits the modern wire in the source world; the legacy emitter
// below serves the frozen oracle — plus the world-agnostic
// `rawBody`/`contentType` transport overrides (byte-frozen CSV exports) —
// with its `{data}` envelopes, epoch ints, 0/1 flags, and the HTTP-200
// `{code: 400}` error emulation the reference build expects.

export const FIXTURE_WORLDS = Object.freeze(['oracle', 'source']);

/** Serialize one fixture response for the given world. */
export function emitFixtureResponse(world, fixture) {
  if (!FIXTURE_WORLDS.includes(world)) {
    throw new Error(`Unknown fixture world "${world}" (expected ${FIXTURE_WORLDS.join(' | ')})`);
  }
  if (fixture?.dialect === 'v2') {
    if (world !== 'source') {
      throw new Error(
        'v2 dialect fixtures are source-world only (the oracle speaks legacy forever)',
      );
    }
    return emitModernFixtureResponse(fixture);
  }
  return emitLegacyFixtureResponse(fixture);
}

/** The legacy wire dialect (the oracle world's permanent shape). */
export function emitLegacyFixtureResponse(fixture) {
  const { contentType = 'application/json', httpStatus = 200, rawBody, ...payload } = fixture;
  if (rawBody !== undefined) {
    return { status: httpStatus, contentType, body: rawBody };
  }
  return { status: httpStatus, contentType, body: JSON.stringify(payload) };
}

/**
 * The modern wire dialect (docs/api-dialect.md §3.1/§4.1): bare JSON bodies,
 * real HTTP statuses (200/201/204), and RFC 9457 problem+json errors. The
 * SPA keys errors on status + `code` only, so the fixture omits the
 * conformance-only `WWW-Authenticate` header the real backend adds to 401s.
 */
export function emitModernFixtureResponse(fixture) {
  const { httpStatus = 200, problem, data } = fixture;
  if (problem) {
    return {
      status: problem.status,
      contentType: 'application/problem+json',
      body: JSON.stringify(problem),
    };
  }
  if (httpStatus === 204) {
    return { status: 204, contentType: 'application/json', body: '' };
  }
  return { status: httpStatus, contentType: 'application/json', body: JSON.stringify(data) };
}
