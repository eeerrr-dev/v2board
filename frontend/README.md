# V2Board Frontend

A pnpm monorepo that reimplements the v2board frontend from scratch against the
original Laravel API (v1). Built fresh — no decompilation, no fork of the old
umi bundle.

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
pnpm install
pnpm dev:user      # http://localhost:5173
pnpm dev:admin     # http://localhost:5174
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```

The dev servers proxy `/api` to the Laravel backend (defaults to
`http://127.0.0.1:8000`, override with `VITE_API_BASE`).

## Deployment

`pnpm build` emits `apps/{admin,user}/dist`. Drop the contents into the existing
Laravel `public/` tree — the user bundle replaces `public/theme/default/` and
the admin bundle replaces `public/assets/admin/`.

## Architecture rules

- TypeScript strict, zero `any`.
- Every HTTP call goes through `@v2board/api-client`.
- User app never imports antd; admin app never imports tailwind.
- TanStack Query owns server state; Zustand owns session/UI state.
- i18next keys are the English string from `resources/lang/en-US.json`.
