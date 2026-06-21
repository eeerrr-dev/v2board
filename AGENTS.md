# AGENTS.md

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## Project Local Docker Workflow

Use Docker for local setup and dependency work. This is a local development
workflow, not a production deployment. Do not run Composer, pnpm, npm, or
build/test commands on the host when they would create `vendor/`,
`node_modules/`, `.pnpm-store/`, `dist/`, `.vite/`, or other cache/build output
inside the repository.

- Use `make up`, `make down`, `make reset`, and `make logs`.
- Use `make doctor` to verify the Compose config, check for host-side
  dependency/cache/build directories, and reject generated or packaged frontend
  files under the host Laravel `public/` targets.
- Use `make public-bundle-audit` when you only need to confirm that
  `public/theme/default/assets/` and `public/assets/admin/` are empty in the
  host working tree.
- Use `make replica-audit` to list runtime/build dependencies on packaged legacy
  frontend bundles. The full replica goal is not complete while this target
  fails.
- Use `make parity-config-audit` after adding, removing, or renaming visual or
  interaction parity scenarios. `make doctor` includes this check so the
  Makefile full-suite lists cannot silently drift from
  `frontend/scripts/visual-parity.mjs` and user/admin routes cannot be added
  without screenshot parity coverage; the check runs in the Docker frontend
  container, not through host Node.
- Use `make legacy-oracle-check` to verify the frozen packaged frontend oracle
  before relying on visual comparisons. It must prove the oracle ref contains
  the complete packaged user/admin entrypoints, old async chunks, i18n scripts,
  and static/theme assets required by `frontend/scripts/visual-parity.mjs`.
- Use `make deploy-smoke` after deploy-path changes to build inside Docker,
  sync `dist-deploy/` into the Docker app container's Laravel `public/` tree,
  and verify source-built user/admin assets are served while old bundle paths
  404.
- Use `make visual-smoke` after visual/layout restoration work. It runs
  `make deploy-smoke`, then opens the source-built Laravel user/admin pages in
  Docker-contained Playwright Chromium across desktop and mobile viewports.
- Use `make visual-parity` only as a read-only oracle check during replica
  work. It extracts the packaged frontend from the git ref stored in
  `frontend/fixtures/legacy-oracle.ref` into Docker `/tmp` and screenshots it
  beside the source-built app; those packaged files must not be copied into
  source, imported by Vite, served by Laravel, or deployed. The check runs in a
  one-off frontend runner container instead of the long-lived dev-server
  container; reports and screenshots are written to a dedicated Docker artifact
  volume mounted at `/app/frontend/.cache/visual-parity`. To keep Docker memory bounded, the
  unfiltered target automatically shards by scenario and viewport. It verifies
  the source-built public assets on port 8000 before the run, then rechecks or
  rebuilds them after exit 137 or an unexpected shard failure before retrying.
  Set `VISUAL_PARITY_CHECK_EACH_SHARD=1` only for especially unstable Docker
  sessions. Each shard pauses memory-heavy services while it runs, then
  restores only the core Laravel app, MySQL, Redis, and Mailpit services
  afterward. Mailpit is intentionally not paused by the visual/interaction
  parity defaults because the Laravel app depends on it and it is lightweight.
  Run `make up` when
  the long-lived frontend dev server, Horizon, or scheduler are needed. Keep the
  Laravel app, MySQL, and Redis containers running so the source-built HTML and
  assets on port 8000 remain available. The short shard delay is intentional to
  let Docker reclaim browser memory between shards.
  Visual oracle runners use a fast dependency bootstrap: they skip `pnpm
  install` when Docker volume `node_modules` already contains the needed
  binaries, but still install inside Docker if dependencies are missing.
- Use `make interaction-parity` for browser-level click/input behavior checks
  against the same frozen packaged oracle. It runs through Docker and writes
  reports to a dedicated Docker artifact volume mounted at
  `/app/frontend/.cache/interaction-parity`; like visual parity, the old bundle
  is extracted only into Docker `/tmp`. This target shards by
  interaction and viewport to keep Chromium memory bounded.
- To run one interaction shard intentionally, narrow the outer Makefile list,
  for example `INTERACTION_PARITY_SCENARIOS=user-node-table-scroll make
  interaction-parity`. `VISUAL_PARITY_INTERACTION_FILTER` is an inner runner
  filter and does not shorten the outer `make interaction-parity` scenario
  loop by itself.
- Focused `VISUAL_PARITY_MODE=interactions make visual-parity` runs without a
  viewport filter are also sharded by viewport by the Makefile. Use
  `make interaction-parity` for the full interaction suite.
- Docker-contained Playwright/oracle runners should access the Laravel source
  deployment through `http://host.docker.internal:8000`, the host-forwarded app
  port. This avoids Compose DNS alias races seen with one-off frontend runners.
  Do not use `http://app:8000` for Chromium checks; Chrome may upgrade the `app`
  host to HTTPS and produce false `Unsupported SSL request` failures. Override
  `VISUAL_SOURCE_BASE_URL` only when debugging a non-Compose source target. The
  Makefile defaults visual parity to a low-memory mode: it pauses frontend,
  horizon, and scheduler while keeping the app, MySQL, Redis, and Mailpit
  running, and sets `VISUAL_PARITY_FRESH_BROWSER=auto` so Chromium is closed
  between source and oracle captures except for the desktop admin dashboard,
  which uses a shared Chromium process because that route is more stable in
  tight Docker memory.
- The local Docker seed/config bootstrap is intentionally idempotent and should
  keep the local admin path fixed at `/admin` with `admin@local / 12345678`.
  Do not let local visual checks depend on the fallback `hash(config('app.key'))`
  admin path; app workspace rebuilds can otherwise change the path while MySQL
  data remains.
- Use `make legacy-oracle-up` when a persistent manual old-frontend browser
  oracle is needed on `http://localhost:8001`; stop it with
  `make legacy-oracle-down`. Use `make legacy-oracle-serve` only for a
  foreground temporary oracle. Both serve the packaged frontend from the frozen
  oracle ref through fixture API responses. The oracle still lives under Docker
  `/tmp` or Docker volume storage; do not copy it into `public/`, Vite sources,
  deploy output, or Laravel runtime views. The foreground serve target may
  pause memory-heavy Compose services while it owns port 8001, then restore the
  core Docker services when the oracle process exits. The manual admin oracle
  is `http://localhost:8001/admin#/login`; use `admin@local / 12345678`. Its
  fixture API infers admin responses from the `/admin` page referrer, so direct
  API probes without that referrer may return user-shaped login data. `make
  down`, `make reset`, and `make sync` remove the named persistent oracle before
  touching Docker volumes.
- For repeated visual debugging without source/deploy changes,
  `VISUAL_PARITY_SKIP_DEPLOY=1 make visual-parity` may reuse the current Docker
  app public assets; `make deploy-public-check` verifies that the source-built
  user/admin assets are currently available on port 8000, and resyncs the
  existing Docker `dist-deploy/` artifact volume into the app `public/` tree if
  an app restart dropped those files. If Docker has also dropped the deploy
  artifact volume, sharded parity falls back to `make deploy-smoke` to rebuild
  the source-built assets before continuing. Use the default `make
  visual-parity` or `make deploy-smoke` after any source, deploy-path, or asset
  change.
- For focused visual debugging, filter visual parity by scenario and viewport,
  for example `VISUAL_PARITY_FILTER=admin-server-manage
  VISUAL_PARITY_VIEWPORT_FILTER=mobile VISUAL_PARITY_SKIP_DEPLOY=1 make
  visual-parity`. A filtered pass is only a focused debugging check, not proof
  that the full replica is complete. Filtered and sharded visual parity runs
  clean leftover frontend one-off runners and automatically retry exit 137,
  which is usually a Docker memory spike rather than visual mismatch.
- Use `make clean-host` to preview ignored host cleanup and `make
  clean-host-apply` only when intentionally removing ignored local artifacts.
- The Makefile intentionally uses `docker-compose.local.yml`, which is tracked.
- `docker-compose.local.yml` mounts source read-only and keeps `.env`, Composer
  vendor files, pnpm store, Playwright browser cache, and frontend
  `node_modules` in Docker volumes. Source-built deploy artifacts live in the
  dedicated Docker `frontend-deploy` volume mounted at
  `/app/frontend/dist-deploy`; visual and interaction reports live in
  dedicated Docker artifact volumes mounted under `/app/frontend/.cache/`.
  Do not write `dist-deploy/` or parity reports on the host.
- The frontend container must not mount `./public` or read packaged frontend
  assets from `/app/public`; source-owned assets live under `frontend/apps/*/src`.
  Do not add new code that depends on old `umi.js`, `umi.css`,
  `components.chunk.css`, `vendors.async.js`, `components.async.js`,
  or `env.example.js`. `custom.css` and `custom.js` may exist only as optional
  operator-provided hooks loaded by the Blade templates; deploy scripts must not
  generate them or copy them from the packaged public tree.
- `docker-compose.yml` is ignored and only for personal overrides. Do not rely
  on it for the canonical local workflow.
- Source is copied into Docker volumes to keep the host repository clean. After
  source edits, run `make sync` to refresh the app/frontend workspaces while
  preserving dependency volumes. `make sync` restores only the core app, MySQL,
  Redis, and Mailpit services; use `make up` when the long-lived frontend dev
  server, Horizon, or scheduler are needed.
- If tests or package commands are needed, run them inside the appropriate
  Docker service with `docker-compose -p v2board -f docker-compose.local.yml exec ...`.

## Frontend Replica Goal

The frontend target is a complete source-level replica: function, behavior,
visual appearance, layout, routing, persistence, edge cases, and deployment
shape must match the packaged V2Board frontend, but runtime and build output
must not depend on the packaged legacy bundles.

Treat this as a strict completion standard, not a directionally-correct
approximation. The restored frontend is not complete until it looks the same,
lays out the same, behaves the same, persists state the same, routes the same,
handles edge cases the same, and can be used the same way as the frozen
packaged oracle. Do not downgrade obvious mismatches to acceptable drift. The
implementation should also be the cleanest mature source restoration available:
organized source code, source-owned assets, no hidden runtime dependency on the
old bundles, no host-generated artifacts, and no temporary bridge presented as
final work.

- The old packaged files may be inspected only as a parity oracle or test
  fixture while restoring source behavior. That means ad hoc comparisons against
  git history, checked fixtures, or `make visual-parity` are allowed; app
  entrypoints, Vite configs, Laravel views, deploy scripts, and production
  bundles must never import, concatenate, copy, or fetch those packaged files.
- The legacy oracle is pinned by `frontend/fixtures/legacy-oracle.ref`, not by
  the current `HEAD`. Keep that ref pointing at a commit that contains the old
  packaged `public/theme/default` and `public/assets/admin` trees. Port 8000 is
  always the source-built Laravel deployment target; port 8001 is an optional,
  temporary manual oracle from `make legacy-oracle-serve`; automated oracle
  screenshots use `make visual-parity`. In both cases the old frontend is only
  restored into Docker `/tmp`. Do not remove or rewrite that oracle ref unless
  you replace it with an equally complete packaged frontend oracle first.
- A monolithic stylesheet reconstructed from a packaged bundle is only a
  temporary recovery layer, not proof of source restoration. Keep it inside
  `frontend/apps/*/src`, make the temporary nature explicit, and replace it with
  organized source styles as the corresponding surface is restored.
- Deployed files may keep legacy-compatible names such as `umi.css` and `umi.js`,
  but those files must be freshly built from `frontend/apps/*/src`, not copied
  from the packaged public tree.
- Final runtime and deploy builds must not load or concatenate old
  `public/theme/default/assets/umi.js`, `umi.css`, `components.chunk.css`,
  `vendors.async.js`, `components.async.js`, `env.example.js`, static/i18n/theme
  assets copied from the packaged public tree, or their admin equivalents.
  Optional `custom.css` and `custom.js` hooks must remain operator-provided and
  must not be emitted by the source deploy build.
- Do not commit or keep generated frontend bundles under
  `public/theme/default/assets/` or `public/assets/admin/` in the host working
  tree; `make public-bundle-audit` must pass. Deploy `dist-deploy/` with
  delete-sync semantics such as `rsync -a --delete`.
- Any temporary bridge to packaged assets must be named as temporary in comments
  and removed as the corresponding source implementation lands.
- Do not claim the replica is complete unless `make replica-audit` and relevant
  `make deploy-smoke` / `make visual-smoke` checks are clean, and broader
  browser-level visual/behavior comparisons prove parity.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
