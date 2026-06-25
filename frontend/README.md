# V2Board Frontend

A pnpm monorepo for restoring the V2Board frontend as source code against the
original Laravel API (v1). The target is a complete source-level replica:
function, behavior, visual appearance, layout, routing, persistence, and deploy
shape should match the packaged frontend exactly, without runtime or build-time
dependency on the packaged legacy bundles.

Current status: the React/TypeScript application source owns its runtime CSS,
fonts, deploy Blade/templates, and imported image assets. Run `make
replica-audit` and `make public-bundle-audit` from the repository root before
claiming the frontend is free of packaged runtime/build dependencies and host
public bundle pollution.

## Layout

```
apps/
  admin/      Ant Design 6 admin panel
  user/       Tailwind + shadcn user panel
packages/
  types/      Shared domain types
  api-client/ Typed Axios client wrapping every documented endpoint
  i18n/       i18next bootstrap + translation tables (zh-CN/zh-TW/en/ja/fa/ru)
  config/     Shared Vite/Tailwind/TS base
```

## Quick start

```bash
make up
make sync          # after source edits, refresh Docker workspaces
make doctor        # verify Compose config and host cleanliness
```

Run dependency, test, build, and Playwright commands inside Docker. Do not run
Composer, pnpm, npm, build, or test commands on the host when they would create
`vendor/`, `node_modules/`, `.pnpm-store/`, `dist/`, `.vite/`, or cache output
inside the repository.

```bash
docker-compose -p v2board -f docker-compose.local.yml exec frontend pnpm test
docker-compose -p v2board -f docker-compose.local.yml exec frontend pnpm typecheck
```

Local ports:

- `http://localhost:5173` is the source user dev server.
- `http://localhost:5174` is the source admin dev server.
- `http://localhost:8000` is the Laravel deployment target using source-built
  assets.
- `make legacy-oracle-serve` starts the old packaged frontend temporarily on
  `http://localhost:8001` from `frontend/fixtures/legacy-oracle.ref`.

The old packaged frontend must not be restored into the current `public/` tree
for comparison. Use `make public-bundle-audit` to verify those host deploy
targets are empty, `make visual-parity` for automated oracle screenshots, and
`make legacy-oracle-serve` only for manual inspection.
Before relying on the oracle, `make legacy-oracle-check` verifies that the
frozen ref contains the packaged user/admin entrypoints, async chunks, i18n
scripts, and static/theme assets used by the visual parity harness.

## Deployment

The deploy build emits `dist-deploy/`. For local verification, run `make
deploy-smoke` from the repository root so the build happens inside Docker. In
CI or a production build environment, run the equivalent `pnpm build:deploy`
there, not on the local host workspace. Deploy with delete-sync semantics so old
packaged files cannot survive beside the source-built bundle:

```bash
rsync -a --delete dist-deploy/theme/default/ /path/to/v2board/public/theme/default/
rsync -a --delete dist-deploy/assets/admin/ /path/to/v2board/public/assets/admin/
```

The repository does not track generated frontend files under
`public/theme/default/assets/` or `public/assets/admin/`. Those directories are
deployment targets only. The deploy output intentionally keeps legacy-compatible
entry names such as `umi.css` and `umi.js`, but those files must come from the
current source build, never from the packaged public tree.

During restoration work, the old packaged CSS/JS may be inspected only as a
parity oracle. It must not become a runtime, Vite, Laravel, or deploy input. Any
monolithic CSS recovered into `frontend/apps/*/src` is a temporary compatibility
layer to be broken back into maintainable source styles, not a completed source
restoration.

When an observed packaged behavior is questionable, classify it with
`docs/legacy-frontend-behavior-audit.md` before copying, correcting, or testing
it. The old frontend is an oracle for compatibility, not the only quality
standard.

For a local end-to-end deployment smoke, run `make deploy-smoke` from the
repository root. It builds in Docker, syncs `dist-deploy/` into the Docker app
container's Laravel `public/` tree, and verifies that the source-built user and
admin resources are served while old bundle paths return 404.

For a browser-rendered smoke, run `make visual-smoke`. It first runs the deploy
smoke, then opens the source-built Laravel user/admin pages in Docker-contained
Playwright Chromium at desktop and mobile sizes. This is a smoke guard for
loaded CSS, visible layout, old-chunk regressions, and horizontal overflow; it
is not by itself proof of full pixel parity with the packaged frontend.

For screenshot parity work, run `make visual-parity`. This command is an oracle
only: it extracts the packaged frontend from git history into Docker `/tmp`,
serves it from a temporary local server, and compares screenshots against the
source-built Laravel deployment. It must not be used as a source, build, Vite,
Laravel, or deployment dependency.

Run `make parity-config-audit` after adding or renaming parity scenarios. It is
also part of `make doctor` and prevents the Makefile full-suite lists from
drifting away from `frontend/scripts/visual-parity.mjs`; it also checks that
user/admin routes have screenshot parity coverage. The target runs in the Docker
frontend container, so it does not require host Node.

The deploy build is not considered fully source-restored while it copies,
concatenates, or links old `umi.css`, `components.chunk.css`, `umi.js`,
`vendors.async.js`, `components.async.js`, `env.example.js`, or packaged
static/i18n/theme assets from `public/`. Optional `custom.css` and `custom.js`
hooks are operator-provided files; the source deploy build must not generate or
copy them from the packaged public tree.

## Architecture rules

- TypeScript strict, zero `any`.
- Every HTTP call goes through `@v2board/api-client`.
- User app never imports antd; admin app never imports tailwind.
- TanStack Query owns server state; Zustand owns session/UI state.
- i18next keys are the English string from `resources/lang/en-US.json`.
