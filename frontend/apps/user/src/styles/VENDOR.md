# User styles — legacy-compatibility CSS vs. authored source

This directory holds the user app's styles. The user frontend is migrated
surface-by-surface into pure shadcn/Radix islands. Legacy styles may remain only
where a route still needs behavior parity or a stable compatibility hook; they
are not the target design system for redesigned user surfaces.

Every file is classified explicitly below — there is no header-marker
convention. When files are added, merged, or renamed, update these lists (and
`CLEANUP-BACKLOG.md` when a deferred cleanup lands).

## 1. Legacy-compatibility CSS (restored from the packaged frontend)

These rules were restored from the frozen oracle bundle so server-rendered
knowledge/notice/plan/homepage HTML keeps its shipped rendering. They are no
longer byte-verbatim: provably dead rules (cascade-dead declarations, class
halves with zero markup consumers) have been conservatively pruned, and the 20
former micro-files were merged. Treat any further rule-level change as a
conscious Tier-2 redesign decision, not routine editing — the packaged frontend
(`frontend/fixtures/legacy-oracle.ref`) remains the reference for what these
rules originally were.

| File | Carries |
| --- | --- |
| `user-custom-html.css` | rich-content rendering for backend HTML (announcements, knowledge, plan content, homepage), scoped under `.custom-html-style`; merged from the former 14 `user-custom-html-*.css` fragments |
| `user-headings.css` | bare `h1`-`h6` document rhythm/scale/color; merged from the former heading base/scale/native-color trio |
| `user-prose-elements.css` | bare `small`/`p`/`b`/`strong` document defaults |
| `user-document-root.css` | `html`/`body`/`#root` document defaults and the dark body base |
| `user-link-elements.css` | bare/classless anchor defaults (`--legacy-*` link colors) |
| `user-browser-modes.css` | document-level `::selection` |

The whole Bootstrap/OneUI **framework** layer (`user-bootstrap-*`,
`user-oneui-*`, and the `user-background-utilities.css` helper) has been
deleted: no surface imported it, no rendered markup carried its
`btn`/`form-control`/`block`/`hero` class names, and Vite never bundled it. The
user app's runtime is modern React with **no antd runtime dependency**; Font
Awesome and Simple Line Icons are retired from the user bundle. New or
redesigned user UI should use `lucide-react` icons and local shadcn-style
primitives. `styles-reachability.test.ts` walks the `@import` graph from
`main.tsx` and fails if an orphaned stylesheet returns;
`test/read-user-styles.ts` enumerates this directory so `globals.test.ts` scans
every file — including new ones — for banned legacy selectors.

Versions/rules are intentionally **not** modernized casually: changing how this
backend HTML renders is a redesign decision. Redesigned shadcn islands must stay
off the old framework CSS and verify behavior/interaction contracts.

## 2. Authored V2Board source styles (source-owned)

These are V2Board's own styling and may be edited normally:

- `user-shadcn.css` — island Tailwind entry: Inter `@font-face`, `@theme` maps,
  `@source` scanning, utility emission
- `user-shadcn-motion.css` — island motion/keyframes
- `user-auth-surface.css` — the **app-wide** island theming file despite its
  historical name: the `.v2board-island` token map and base, the `.dark` flip,
  the prose neutralizer, and the dark rich-content overrides
- `user-theme-colors.css`, `user-theme-legacy-tokens.css` — hand-curated
  survivors of the packaged token sheets; only tokens with a live var()/runtime
  consumer remain
- `globals.css` — Tailwind preflight entry (`source(none)`)
- `user-legacy-replica.css`, `user-redesigned-surfaces.css` — import manifests
  (the order in `user-redesigned-surfaces.css` is load-bearing; see its header)
