# User-surface audit backlog (deferred cleanups)

Follow-ups from the 2026-06-30 user-surface shadcn audit. The safe, authored-source
cleanups already landed (commit `refactor(user): drop dead CSS remnants from shadcn
island surfaces`). Items 1-3 below were **executed on 2026-07-02** as part of the
styles-layer audit fixes; item 5 was **executed on 2026-07-09** (semantic status
tokens). Item 4 remains a deliberately deferred design decision (a review-gated
homepage-typography redesign). None of the open items have a visual or contract
impact today.

Severity legend: all items are **nit** — none block the "clean / modern shadcn"
verdict. Ordered by how self-contained the fix is.

---

## 1. ~~`--color-heading` is a cascade-dead token with a misleading "live" comment~~ — DONE (2026-07-02)

- **Was:** `user-heading-base.css`'s `color: var(--color-heading)` was always
  out-cascaded by `user-heading-native-color.css` (`rgba(0, 0, 0, 0.85)`) at equal
  specificity, so the `--color-heading: #171717` token in `user-theme-colors.css`
  never painted, yet its comment listed it as live. Stale `#171717` mentions also
  lingered in `user-auth-surface.css` comments.
- **Done:** the dead `var(--color-heading)` declaration and the token are deleted;
  the merged [`user-headings.css`](./user-headings.css) owns the single bare-heading
  color (`rgba(0, 0, 0, 0.85)`), and the stale comments in
  [`user-theme-colors.css`](./user-theme-colors.css) and
  [`user-auth-surface.css`](./user-auth-surface.css) are fixed.

## 2. ~~Dead `.h1`-`.h6` / `.small` class-selector halves in vendored heading/prose CSS~~ — DONE (2026-07-02)

- **Was:** Bootstrap-heritage class selectors (`.h1`..`.h6`, `.small`) had zero
  markup consumers; only the paired bare-element halves ever matched.
- **Done:** the class halves are dropped (a conscious Tier-2 choice — operator
  `.custom-html-style` HTML could theoretically have used the Bootstrap classes);
  only the bare-element selectors remain, in
  [`user-headings.css`](./user-headings.css) and
  [`user-prose-elements.css`](./user-prose-elements.css).

## 3. ~~Consolidate the 14 `user-custom-html-*.css` micro-files~~ — DONE (2026-07-02)

- **Was:** 14 single-purpose files shared one selector prefix and one purpose and
  were never imported independently.
- **Done:** merged into [`user-custom-html.css`](./user-custom-html.css) in the
  original import order, dropping only the cascade-dead table-cell
  `word-break: break-all` line (the later header/body cell rules always overrode it)
  while keeping the live `word-wrap`/`white-space` declarations. The 3 heading files
  were likewise collapsed into [`user-headings.css`](./user-headings.css).
  `VENDOR.md` §1 was rewritten to match (the files are no longer byte-verbatim;
  rule-level changes remain conscious redesign decisions).

## 4. (Optional) Scope global bare-element prose under `.custom-html-style` / homepage

- **Files:** [`user-headings.css`](./user-headings.css),
  [`user-prose-elements.css`](./user-prose-elements.css) (bare `h1-h6`/`p`/`small`),
  neutralized inside islands by
  [`user-auth-surface.css`](./user-auth-surface.css) `.v2board-island :where(h1..h6,p){margin:0;color:inherit}`.
- **Problem:** these document-wide bare-element rules bleed into every shadcn island, so
  the island layer carries a counter-reset. A maximally-clean setup would scope the prose
  rhythm under `.custom-html-style` (and the public-homepage root) so islands never
  inherit it and the neutralizer could be deleted.
- **Why deferred:** (a) rule-level legacy-compatibility changes are redesign decisions
  (VENDOR.md §1); (b) the public homepage (`home.tsx`) renders `.custom-html-style`
  content **outside any island**, and its fallback branch renders bare `<a>`/`<div>`
  too, so the document-level defaults are a real non-island contract, not purely dead;
  (c) the single-membership-class neutralization is the documented, drift-proof island
  design, not an accident.
- **Proposed change (larger redesign):** move prose/heading rhythm into
  `.custom-html-style` + a homepage wrapper, then remove the island neutralizer. Only
  worth it as part of a homepage redesign.
- **Risk:** medium — touches non-island homepage typography; needs visual review.
- **Gate:** `make visual-parity`/`visual-smoke` on `home` + island dialog/sheet titles;
  `styles/globals.test.ts`.

## 5. ~~Lift status tones into semantic tokens~~ — DONE (2026-07-09)

- **Was:** [`../components/ui/status-badge.tsx`](../components/ui/status-badge.tsx)
  (badge + dot tones), `dashboard.tsx` (alerts + progress bar), **and
  `orders/detail.tsx`** (order-result icon) each hardcoded raw Tailwind light+dark
  ramps for `info`/`success`/`warning`. The backlog originally under-scoped this to
  two files; the third (`orders/detail.tsx`) used a *different* hue family
  (`green`/`yellow`/`blue` vs `sky`/`emerald`/`amber`), so the same semantic state
  rendered in different colors across surfaces — present-day drift, not just
  non-themeability.
- **Done:** added `--success`/`--warning`/`--info` to the island token map
  ([`user-auth-surface.css`](./user-auth-surface.css) light + dark) and mapped them
  through `@theme inline` in [`user-shadcn.css`](./user-shadcn.css). All three
  surfaces now consume `text-<tone>` / `bg-<tone>/10` / `border-<tone>/30` / solid
  dots+bars — one operator-repaintable, drift-free hue per state, matching how
  `--destructive` is already handled. No `-foreground` tokens were added (no
  consumer — dots/bars use the solid tone; badges/icons use it as text). Still
  Tier-2 presentation; the tone shift was a conscious redesign choice.

---

_Generated from the user-surface audit; keep in sync with `VENDOR.md` if the
legacy-compatibility vs. authored classification changes._
