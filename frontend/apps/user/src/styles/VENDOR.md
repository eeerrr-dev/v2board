# User styles — vendored third-party CSS vs. authored source

This directory holds the user app's styles. The user frontend is now being
migrated surface-by-surface into pure shadcn/Radix islands. Legacy styles may
remain only where a route still needs behavior parity or a stable compatibility
hook; they are not the target design system for redesigned user surfaces.

## 1. Vendored third-party CSS (compatibility only)

Some files in this directory are upstream framework stylesheets the original
packaged V2Board user theme shipped, restored from the frozen oracle bundle.
They are NOT authored source and should not be hand-edited rule-by-rule. The
redesigned user runtime no longer imports the Bootstrap/OneUI UI framework
selectors; only document and rich-content compatibility remains in
`user-legacy-replica.css`.

The user app's runtime is modern React and has **no antd runtime dependency**.
The redesigned user app no longer loads Ant Design, antd-mobile, Ant modal/drawer,
or old OneUI sidebar/page-shell presentation CSS. Stable behavior hooks may keep
`v2board-*` names, but new visible UI should use local shadcn-style primitives
and Radix behavior.

The whole Bootstrap/OneUI **framework** layer (`user-bootstrap-*`,
`user-oneui-*`, and the `user-background-utilities.css` helper) has been deleted:
no surface imported it, no rendered markup carried its `btn`/`form-control`/
`block`/`hero` class names, and Vite never bundled it. The frozen packaged
frontend remains the parity oracle (`frontend/fixtures/legacy-oracle.ref`); these
in-tree copies were redundant compatibility CSS, not the oracle. Only the
document/rich-content compatibility files below survive, because server-rendered
knowledge/notice HTML still relies on them. `styles-reachability.test.ts` walks
the `@import` graph from `main.tsx` and fails if an orphaned stylesheet returns.

| Filename prefix | Upstream library | Version |
| --- | --- | --- |
| `user-custom-html-*`, `user-prose-*`, `user-heading-*` | knowledge/markdown prose rendering (OneUI + Bootstrap typography), scoped under `.custom-html-style` | as shipped |
| `user-document-root.css`, `user-link-elements.css`, `user-browser-modes.css` | restored document-level defaults (body/link/selection) | as shipped |
Font Awesome and Simple Line Icons are retired from the user bundle. New or
redesigned user UI should use `lucide-react` icons when an icon is appropriate.

Versions are intentionally **not** upgraded casually: changing them would be a
redesign decision. Redesigned shadcn islands should stay off the old framework
CSS and verify behavior/interaction contracts.

## 2. Authored V2Board source styles (source-owned)

Every file whose header begins `/* Authored V2Board …` is V2Board's own styling
(identifiable by `___`-hashed CSS-module classes or `.v2board-*` selectors). These
are the organized source layer and may be edited normally. In this app they are:

- `user-shadcn.css`, `user-auth-surface.css`, `user-shadcn-motion.css` — pure shadcn island tokens, utilities, and motion

## How to classify a file here

A style file is **authored V2Board source** iff its header says `Authored V2Board`
(it contains `___` module classes or `.v2board-` selectors). Everything else is
**vendored** third-party CSS restored verbatim from the packaged frontend.
