/**
 * Deploy-seam constants shared by the build, smoke, and source-audit scripts.
 *
 * The same contract is independently encoded on the Rust side (deliberate
 * defense in depth: Rust must validate any installed tree, builder-produced or
 * not) and in the Makefile. `make deploy-contract-audit` fails when any copy
 * drifts:
 * - backend/rust/crates/api/src/frontend.rs            RUNTIME_CONFIG_TOKEN,
 *                                       USER_/ADMIN_PREPAINT_SCRIPT_HASH
 * - backend/rust/crates/provision/src/release_archive.rs is_forbidden_legacy_filename
 * - Makefile                                           FORBIDDEN_LEGACY_NAMES
 * The apps' index.html templates and their vite-config pin tests keep the raw
 * token literal on purpose — the template is the artifact being validated.
 */

export const runtimeConfigToken = '__V2BOARD_RUNTIME_CONFIG__';

/**
 * Server-substituted head branding literals: the Rust renderer replaces each
 * one per request with operator-configured values (frontend.rs
 * USER_/ADMIN_TITLE_TOKEN, DESCRIPTION_TOKEN, HEAD_META_TOKEN). The dev
 * templates keep human-readable defaults; build-deploy.mjs asserts each
 * literal survives the build exactly once, and the social-meta marker stays
 * user-only — the admin document ships a static noindex instead.
 */
export const documentTitleTokens = Object.freeze({
  user: '<title>V2Board</title>',
  admin: '<title>V2Board Admin</title>',
});
export const descriptionToken = '<meta name="description" content="V2Board" />';
export const headMetaToken = '<!-- __V2BOARD_HEAD_META__ -->';

/**
 * SHA-256 CSP source allowances for each app's single executable inline
 * script — the dark-mode pre-paint (docs/api-dialect.md §10.5). The apps
 * currently share one script byte-for-byte, but the entries stay per-app
 * because Vite may emit them differently. build-deploy.mjs recomputes each
 * built index.html's hash and fails the build on drift; the Rust document CSP
 * pins the same values (frontend.rs USER_/ADMIN_PREPAINT_SCRIPT_HASH).
 */
export const prepaintScriptHashes = Object.freeze({
  user: 'sha256-xvE7y+NVTYJtOqEHosh/TIUayVxvwstXsS01qdJfcrc=',
  admin: 'sha256-xvE7y+NVTYJtOqEHosh/TIUayVxvwstXsS01qdJfcrc=',
});

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
