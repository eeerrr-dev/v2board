// World-aware fixture emitters (docs/api-dialect.md §13.5).
//
// `fixture-data.mjs` stays the canonical data source; `api-fixtures.mjs`
// serializes every intercepted API response through this per-world seam.
// A fixture response is the legacy-envelope object produced by
// `apiFixtureResponse` — `{code, data, total?, type?, message?}` plus the
// optional `httpStatus`/`rawBody`/`contentType` transport overrides — and an
// emitter turns it into the wire `{status, contentType, body}`.
//
// Per Appendix A §W0 BOTH worlds emit the legacy wire dialect: `{data}`
// envelopes, epoch ints, 0/1 flags, and the HTTP-200 `{code: 400}` error
// emulation the reference build expects. Family waves teach the source
// emitter modern shapes route-by-route (bare objects / `{items,total}`,
// RFC 3339, booleans, real HTTP semantics with problem+json bodies) while
// the oracle emitter keeps legacy forever.

export const FIXTURE_WORLDS = Object.freeze(['oracle', 'source']);

/** Serialize one fixture response for the given world. */
export function emitFixtureResponse(world, fixture) {
  if (!FIXTURE_WORLDS.includes(world)) {
    throw new Error(`Unknown fixture world "${world}" (expected ${FIXTURE_WORLDS.join(' | ')})`);
  }
  // W0: identity — the source world still speaks the legacy dialect.
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
