# Legacy Frontend Behavior Audit

This document is the decision layer for source restoration work. The packaged
frontend is an oracle for compatibility, but it is not the final quality bar.
Every observed legacy behavior must be classified before it is restored,
corrected, or left for a focused decision.

## Classifications

Use these labels in issues, tests, and code comments when a behavior is being
locked down.

| Label | Meaning | Default action |
| --- | --- | --- |
| `legacy-compat` | Operators, users, backend contracts, stored links, deployed paths, or parity checks can reasonably depend on this behavior. | Preserve it and cover it with focused tests or parity scenarios. |
| `legacy-defect-corrected` | The old behavior is invalid, brittle, inaccessible, unsafe, or only an artifact of the packaged implementation. | Correct it in source and cover the intended behavior instead. |
| `case-by-case` | The behavior has visible compatibility value but also creates product or engineering debt. | Keep the current restoration stable, then decide with evidence from oracle screenshots, interaction checks, and stress cases. |

## Preserve As Compatibility

These behaviors are compatibility contracts unless a separate migration plan
proves otherwise.

| Area | Legacy behavior to preserve | Why it matters | Verification |
| --- | --- | --- | --- |
| Routing | Hash routes, old deep links, route aliases, login redirects, admin path assumptions. | Existing users and operator docs can contain old URLs. | Route tests plus visual/interaction parity for the affected pages. |
| API payloads | Request shapes, field names, units, pagination params, and response fallback semantics expected by the Laravel API. | Backend compatibility is more important than a cleaner frontend model. | API-client tests and page flow tests. |
| Auth/session | Token storage, admin/user session separation, logout side effects, and secure admin-path bootstrap behavior. | Session migration bugs can lock users out or mix admin/user state. | Session store tests and login/logout interaction checks. |
| Deploy shape | Legacy-compatible emitted names such as `umi.css` and `umi.js`, while built from current source. | Laravel templates and existing deploy expectations refer to those names. | `make deploy-smoke`, `make replica-audit`, and `make public-bundle-audit`. |
| Operator hooks | Optional `custom.css` and `custom.js` hooks remain operator-provided only. | Operators may depend on these extension points. | Deploy smoke and bundle audits prove the source build does not generate them. |
| i18n persistence | Existing language cookie/localStorage keys and reload semantics where pages already assume a reload. | Changing locale persistence can break saved user preference behavior. | i18n package tests and manual language-switch checks. |
| Ant Design surface | DOM/class/layout behavior that parity scenarios prove is visible to users. | The admin UI is highly class-driven and table-heavy. | Screenshot parity and targeted component tests. |

## Correct Old Defects

These are not standards to blindly copy. Correct them when implementing or
touching the related surface.

| Legacy defect | Correct source behavior | Verification |
| --- | --- | --- |
| Invalid CSS such as `padding-left:auto`. | Replace with the intended valid rule, for example `padding-left: 0` or a layout-specific value. | CSS review, visual smoke, and focused viewport checks. |
| Missing or incomplete document language/direction environment. | Set `html[lang]`, `dir`, `data-locale`, and `data-text-direction` from the active locale. | Unit tests for locale direction and RTL page classes. |
| Hard-coded admin Chinese locale or copied locale assumptions. | Use the shared i18n locale environment once the admin locale surface is restored. | Admin i18n tests and language stress screenshots. |
| Fixed-width tables and nowrap text that break under long translations or mobile viewports. | Keep compatible table geometry where required, but add intentional scroll, wrap, or ellipsis rules per surface. | Mobile visual smoke plus long-language stress scenarios. |
| Random keys used only because the packaged bundle remounted components accidentally. | Use stable keys unless the remount is a visible compatibility behavior. | Component tests proving state/focus is preserved where desired. |
| Uncancelled async effects, module-scoped timers, or global click handlers with no compatibility need. | Scope cleanup to the component lifecycle and avoid cross-page leaks. | Interaction tests around navigation, close/reopen, and unmount flows. |
| Literal implementation artifacts such as accidental `"false"` class names. | Do not propagate the artifact into new abstractions unless a parity check proves the exact class string matters. | Component tests and screenshot parity for affected layout containers. |
| Layout that only works for short English or Chinese labels. | Test long labels, RTL labels, and narrow screens as first-class inputs. | Locale stress tests and mobile screenshots. |

## Case-By-Case Behaviors

These need evidence before changing because they may be both ugly and relied on.

| Behavior | Keep when | Improve when |
| --- | --- | --- |
| Full page reload after language switch. | Old stores or route loaders only become consistent after reload. | The affected app can update locale, document environment, and cached queries safely in-place. |
| Exact old copy and punctuation. | The text appears in screenshots, operator docs, payment flows, or support instructions. | The old copy is mistranslated, ambiguous, or blocks a restored locale from fitting. |
| Table min-widths and horizontal scrolling. | The old table shape is necessary for dense admin data scanning. | It creates avoidable overflow for common mobile or translated views. |
| Ellipsis-only mobile rows. | The old compact list is the only usable density for the page. | Hidden content prevents the expected action or makes translated labels unreadable. |
| Old null/empty fallback display. | Backend responses and user expectations rely on the exact placeholder. | The fallback hides an error, breaks accessibility, or causes layout collapse. |

## Required Audit Workflow

1. Identify the source of the behavior: packaged oracle, existing test, API
   contract, Laravel template, operator hook, or current source code.
2. Classify it as `legacy-compat`, `legacy-defect-corrected`, or
   `case-by-case`.
3. Add a focused test, visual scenario, or interaction scenario that expresses
   the classification.
4. For corrected defects, add at least one stress input when relevant: long
   translated text, RTL locale, mobile viewport, empty data, or slow async
   completion.
5. Keep packaged assets out of runtime, Vite, Laravel views, deploy output, and
   source imports. The oracle may be inspected, never reused as implementation.

## Current Seed Audit

This is the starting inventory from the restoration work so far. It is not a
claim that the full old frontend has been audited.

| Item | Classification | Status |
| --- | --- | --- |
| Source-built deploy files keep legacy entry names. | `legacy-compat` | Guarded by deploy and replica audits. |
| Old packaged public bundles are not copied into source or deploy output. | `legacy-compat` | Guarded by `make public-bundle-audit` and `make replica-audit`. |
| Single locale registry (`packages/i18n/src/locale-registry.ts`) is the only source of locale identity: codes, labels, navigator mapping, i18next resources, reproduced antd-pack strings, and direction all derive from it. | `legacy-compat` | Adding or removing a locale is one registry entry; no other file enumerates the locale set. Covered by i18n unit tests. |
| Locale document environment for `lang` and `dir`. | `legacy-compat` | `lang` and `dir` are derived from the registry entry; every bundled locale is `ltr`. |
| RTL / fa-IR (Persian) support. | `case-by-case` (out of scope) | Intentionally **not** bundled. fa-IR (the oracle's 7th locale) and RTL direction are dropped — a conscious divergence from the oracle's locale set. Operators may still enable fa-IR in `window.settings.i18n`, but the menu drops any locale the registry does not render. The registry `dir` field is the single seam to add an RTL locale later. |
| Fixed-column table row-height sync. | `legacy-defect-corrected` | Synced purely by measuring real row heights, as the bundled rc-table did. The per-locale/per-browser `+1px` offset enumeration was removed. |
| Admin locale hard-coded to Chinese Ant locale. | `case-by-case` | Preserve until admin i18n restoration is scoped, then migrate to shared locale behavior. |
| Fixed-width table layouts across user/admin pages. | `case-by-case` | Needs page-by-page language and viewport stress audit. |
| Random keys and accidental remount behavior in restored flows. | `case-by-case` | Preserve only where current tests describe visible legacy behavior. |
| Uncancelled async effects in login/payment/profile flows. | `case-by-case` | Needs interaction tests before cleanup. |

## Priority Backlog

| Priority | Work | Exit criteria |
| --- | --- | --- |
| P0 | Protect compatibility contracts: routing, auth/session, API payloads, deploy shape, and no packaged runtime dependency. | `make replica-audit`, `make public-bundle-audit`, relevant API/session tests, and deploy smoke pass. |
| P1 | Build a language and viewport stress matrix for user/admin: long English, Traditional Chinese, Japanese, Russian, Persian/RTL, desktop, and mobile. | Focused tests or parity scenarios exist for the high-risk pages and fail on overflow or unreadable controls. |
| P1 | Audit fixed table widths, nowrap buttons, sidebar/header labels, modal forms, and mobile lists. | Each surface has an explicit preserve/improve decision and a verification path. |
| P1 | Restore admin locale behavior through the shared i18n layer. | Admin Ant locale, document environment, and visible copy follow the active locale without breaking old persistence. |
| P2 | Replace accidental remounts, global handlers, and uncancelled async effects where not required for compatibility. | Interaction tests prove no regression in close/reopen, navigation, polling, and login flows. |
| P2 | Split temporary monolithic compatibility styles into source-owned modules. | Styles are organized by surface without changing parity-covered visuals. |

The desired end state is not "new frontend equals old frontend." The desired
end state is: old-compatible where users and deployments depend on it, cleaner
than old where the old behavior was accidental, invalid, or too brittle to keep.
