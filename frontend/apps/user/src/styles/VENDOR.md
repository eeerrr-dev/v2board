# User styles — vendored third-party CSS vs. authored source

This directory holds the user app's styles. They fall into two deliberate
categories. **This split is an accepted final state, not a temporary recovery
layer.**

## 1. Vendored third-party CSS (kept verbatim for visual parity)

These files are the upstream framework stylesheets the original packaged V2Board
user theme shipped, restored from the frozen oracle bundle and **kept byte-faithful
on purpose** so the rewrite matches the oracle pixel-for-pixel. They are NOT authored
source and should not be hand-edited rule-by-rule; if a vendored surface ever needs
to change, re-derive it from the upstream library at the version below and re-run
`make visual-parity`.

The user app's runtime is modern (React 19) and has **no antd dependency at all** —
every visible control is hand-built `.ant-*`/OneUI/Bootstrap DOM styled by the
vendored CSS below. So "old framework" here means *vendored CSS/icon-font look*, not
old running JavaScript.

| Filename prefix | Upstream library | Version |
| --- | --- | --- |
| `user-antd-*` | Ant Design | **3.26.20** (oracle-pinned; the look the rewrite replicates) |
| `user-bootstrap-*` | Bootstrap | **4.x** (bundled via OneUI; exact patch not recovered from the bundle) |
| `user-oneui-*`, `user-sidebar-nav-*`, `user-page-shell-*` | OneUI dashboard template (pixelcave) — layout, sidebar nav, page shell | as shipped in the packaged frontend (release not precisely pinned) |
| `user-custom-html-*`, `user-prose-*`, `user-markdown-*` | knowledge/markdown prose rendering (OneUI + Bootstrap typography) | as shipped |
| `user-font-*` | Font Awesome | **5 (Free)** |

Versions are intentionally **not** upgraded: the project goal is a faithful replica
of the frozen oracle, and the oracle's appearance *is* these framework versions.
Upgrading would change the look and break `make visual-parity` — that would be a
redesign, not a cleanup. See the repo replica goal in `AGENTS.md`.

## 2. Authored V2Board source styles (source-owned)

Every file whose header begins `/* Authored V2Board …` is V2Board's own styling
(identifiable by `___`-hashed CSS-module classes or `.v2board-*` selectors). These
are the organized source layer and may be edited normally. In this app they are:

- `user-auth-shell.css`, `user-auth-alerts.css`, `user-auth-language.css` — guest auth surface
- `user-dashboard-shortcut-items.css`, `user-dashboard-background-pixels.css` — dashboard
- `user-plan-tabs.css`, `user-plan-content-features.css`, `user-plan-stock-tags.css`, `user-plan-coupon-input.css` — plans/checkout
- `user-payment-elements.css`, `user-payment-select.css` — payment/cashier
- `user-order-info.css`, `user-trade-number.css` — order detail
- `user-subscribe-list.css`, `user-ticket-chat-legacy.css` — one-click subscribe list, ticket chat (`___` module classes)
- `user-mobile-search-overrides.css`, `user-mobile-select-overrides.css` — mobile overrides
- `user-email-whitelist-enable.css`, `user-antd-radio-native-select-bridge.css`, `user-shell-polish.css`, `user-sidebar-nav-footer-mask.css` — misc app overrides

## How to classify a file here

A style file is **authored V2Board source** iff its header says `Authored V2Board`
(it contains `___` module classes or `.v2board-` selectors). Everything else is
**vendored** third-party CSS restored verbatim from the packaged frontend. Note the
`user-mobile-*` prefix is mixed — trust the header, not the prefix.
