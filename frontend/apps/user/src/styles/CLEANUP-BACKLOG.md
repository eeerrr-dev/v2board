# User-surface audit backlog (deferred cleanups)

Follow-ups from the 2026-06-30 user-surface shadcn audit. The safe, authored-source
cleanups already landed (commit `refactor(user): drop dead CSS remnants from shadcn
island surfaces`). The items below were **deliberately deferred**: each requires
editing vendored-verbatim CSS that [`VENDOR.md`](./VENDOR.md) §1 says "should not be
hand-edited rule-by-rule", or is a design decision rather than a defect. None have a
visual or contract impact today. Treat each as a conscious redesign decision.

Severity legend: all items are **nit** — none block the "clean / modern shadcn"
verdict. Ordered by how self-contained the fix is.

---

## 1. `--color-heading` is a cascade-dead token with a misleading "live" comment

- **Files:** [`user-theme-colors.css:19`](./user-theme-colors.css) (declares
  `--color-heading: #171717`), [`user-theme-colors.css:9`](./user-theme-colors.css)
  (documents it as a *live* "global h1-h6 color" consumer),
  [`user-heading-base.css:14`](./user-heading-base.css) (`color: var(--color-heading)`),
  [`user-heading-native-color.css`](./user-heading-native-color.css)
  (`h1..h6 { color: rgba(0,0,0,0.85) }`).
- **Problem:** `user-heading-native-color.css` is imported *after* `user-heading-base.css`
  at equal specificity, so `rgba(0,0,0,0.85)` always wins on real headings — the
  `var(--color-heading)` declaration never paints. Inside islands, headings are reset to
  `color: inherit`. So the token is effectively dead, yet `user-theme-colors.css`'s
  curated "still-live tokens" comment lists it as live. Stale `#171717` mentions also
  linger in comments at [`user-auth-surface.css:57`](./user-auth-surface.css) and
  [`user-auth-surface.css:107`](./user-auth-surface.css).
- **Why deferred:** the only `var()` reference lives in vendored `user-heading-base.css`.
  Removing the token from the authored `user-theme-colors.css` alone would leave a
  dangling `var(--color-heading)` in vendored CSS; a clean fix needs a rule-by-rule edit
  of a vendored file (VENDOR.md §1).
- **Proposed change (redesign decision):** drop `color: var(--color-heading)` from
  `user-heading-base.css`, delete `--color-heading` from `user-theme-colors.css`, let
  `user-heading-native-color.css` own the single bare-heading color, and fix the stale
  `#171717` comments.
- **Risk:** none visually (native-color already paints the intended near-black; islands
  reset to `inherit`).
- **Gate:** `styles/globals.test.ts`, `styles/styles-reachability.test.ts`, focused
  `make visual-parity`/`visual-smoke` on the public homepage (only non-island headings).

## 2. Dead `.h1`-`.h6` / `.small` class-selector halves in vendored heading/prose CSS

- **Files:** [`user-heading-base.css:2-7`](./user-heading-base.css),
  [`user-heading-scale.css`](./user-heading-scale.css) (`.h1`..`.h6` at lines 2,7,12,17,22,27),
  [`user-prose-elements.css:3`](./user-prose-elements.css) (`.small`).
- **Problem:** Bootstrap-heritage class selectors (`.h1`..`.h6`, `.small`) have zero
  markup consumers — a grep of `pages/**` + `components/**` finds no such `className`.
  Only the paired bare-element halves (`h1`..`h6`, `small`) ever match. The class halves
  are inert, slightly inflating the document-global selector surface.
- **Why deferred:** these are vendored-verbatim files (VENDOR.md §1); dropping the class
  halves is exactly the rule-by-rule edit the policy discourages.
- **Proposed change (redesign decision):** drop the `.h1`-`.h6` / `.small` class halves,
  keep only the bare-element selectors that real markup/backend-HTML uses.
- **Risk:** none (the class selectors match nothing).
- **Gate:** `styles/globals.test.ts`, `styles/styles-reachability.test.ts`.

## 3. Consolidate the 14 `user-custom-html-*.css` micro-files

- **Files:** [`user-legacy-replica.css:16-29`](./user-legacy-replica.css) imports 14
  single-purpose `user-custom-html-*.css` files (some ~113 B / one rule), all scoped
  under `.custom-html-style`.
- **Problem:** 14 files share one selector prefix and one purpose (rendering backend
  knowledge/notice/homepage HTML) and are never imported independently. The fine-grained
  split adds `@import` and cognitive overhead with no modularity benefit.
- **Why deferred:** vendored rich-content CSS (VENDOR.md §1); organizational-only, and
  `styles-reachability.test.ts` (which walks the `@import` graph) keeps every rule
  guarded whether split or merged.
- **Proposed change (redesign decision):** merge into one `user-custom-html.css` (or a
  few coherent groups). Purely an elegance/maintainability win; all rules stay reachable.
- **Risk:** none functional; verify the concatenated CSS byte-for-byte matches the
  `globals.test.ts` `.custom-html-style` assertions.
- **Gate:** `styles/globals.test.ts`, `styles/styles-reachability.test.ts`.

## 4. (Optional) Scope global bare-element prose under `.custom-html-style` / homepage

- **Files:** [`user-heading-base.css`](./user-heading-base.css),
  [`user-heading-scale.css`](./user-heading-scale.css),
  [`user-prose-elements.css`](./user-prose-elements.css) (bare `h1-h6`/`p`/`small`),
  neutralized inside islands by
  [`user-auth-surface.css`](./user-auth-surface.css) `.v2board-island :where(h1..h6,p){margin:0;color:inherit}`.
- **Problem:** these document-wide bare-element rules bleed into every shadcn island, so
  the island layer carries a counter-reset. A maximally-clean setup would scope the prose
  rhythm under `.custom-html-style` (and the public-homepage root) so islands never
  inherit it and the neutralizer could be deleted.
- **Why deferred:** (a) vendored files (VENDOR.md §1); (b) the public homepage
  (`home.tsx`) renders `.custom-html-style` content **outside any island**, and its
  fallback branch renders bare `<a>`/`<div>` too, so the document-level defaults are a
  real non-island contract, not purely dead; (c) the single-membership-class
  neutralization is the documented, drift-proof island design, not an accident.
- **Proposed change (larger redesign):** move prose/heading rhythm into
  `.custom-html-style` + a homepage wrapper, then remove the island neutralizer. Only
  worth it as part of a homepage redesign.
- **Risk:** medium — touches non-island homepage typography; needs visual review.
- **Gate:** `make visual-parity`/`visual-smoke` on `home` + island dialog/sheet titles;
  `styles/globals.test.ts`.

## 5. (Optional) Lift status-badge tones into semantic tokens

- **File:** [`../components/ui/status-badge.tsx`](../components/ui/status-badge.tsx)
  lines 13/15/17 (badge tones) and 28-30 (dot tones) hardcode raw Tailwind
  `sky`/`emerald`/`amber` light+dark ramps for `info`/`success`/`warning`.
- **Problem:** every other primitive routes color through the island `@theme` token map
  (`--primary`/`--muted`/`--destructive`); status-badge reaches around it to fixed
  palette literals, so its tones can't be operator-repainted and can drift from the
  design system. (Same literals are mirrored in `dashboard.tsx`.)
- **Why deferred / not a defect:** shadcn/ui ships **no** `--success`/`--warning`/`--info`
  tokens by default, so hardcoding the ramps is the canonical shadcn approach; the code is
  correct and fully dark-mode complete. Status-badge rendering is explicitly Tier-2
  presentation in `AGENTS.md` (not operator-themeable).
- **Proposed change (design decision):** introduce `--success/--warning/--info`
  (+`-foreground`) tokens in the island `@theme` map (`user-shadcn.css` /
  `user-auth-surface.css` light+dark) and reference them here and in `dashboard.tsx`.
- **Risk:** low; purely additive theming.
- **Gate:** `status-badge`/`dashboard` vitest, dark-mode visual check.

---

_Generated from the user-surface audit; keep in sync with `VENDOR.md` if the vendored vs.
authored classification changes._
