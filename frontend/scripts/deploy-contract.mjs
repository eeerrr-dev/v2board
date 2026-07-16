/**
 * Deploy-seam constants shared by the build, smoke, and source-audit scripts.
 *
 * The same contract is independently encoded on the Rust side (deliberate
 * defense in depth: Rust must validate any installed tree, builder-produced or
 * not) and in the Makefile. `make deploy-contract-audit` fails when any copy
 * drifts:
 * - backend/rust/crates/api/src/frontend.rs            RUNTIME_CONFIG_TOKEN
 * - backend/rust/crates/provision/src/release_archive.rs is_forbidden_legacy_filename
 * - Makefile                                           FORBIDDEN_LEGACY_NAMES
 * The apps' index.html templates and their vite-config pin tests keep the raw
 * token literal on purpose — the template is the artifact being validated.
 */

export const runtimeConfigToken = '__V2BOARD_RUNTIME_CONFIG__';

export const forbiddenLegacyNames = Object.freeze([
  'components.chunk.css',
  'vendors.async.js',
  'components.async.js',
  'custom.css',
  'custom.js',
  'env.example.js',
  'umi.css',
  'umi.js',
]);

/**
 * Hashed asset filename grammar certified at build time: one hashed flat name
 * with a single non-dotted extension segment. Deliberately a strict subset of
 * the runtime serving gate in backend/rust/crates/api/src/routes.rs
 * (is_content_hashed_asset), so every build-certified name is provably
 * runtime-servable. Both sides exercise the same filename corpus —
 * scripts/deploy-contract.test.mjs here, the routes tests on the Rust side.
 */
export const hashedAssetNamePattern = /^[A-Za-z0-9._-]+-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9]+$/;
