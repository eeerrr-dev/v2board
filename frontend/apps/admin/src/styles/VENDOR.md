# Admin styles — vendored third-party CSS vs. authored source

This directory holds the admin app's styles. They fall into two deliberate
categories. **This split is an accepted final state, not a temporary recovery
layer.**

## 1. Vendored third-party CSS (kept verbatim for visual parity)

These files are the upstream framework stylesheets the original packaged V2Board
admin shipped, restored from the frozen oracle bundle and **kept byte-faithful on
purpose** so the rewrite matches the oracle pixel-for-pixel. They are NOT authored
source and should not be hand-edited rule-by-rule; if a vendored surface ever needs
to change, re-derive it from the upstream library at the version below and re-run
`make visual-smoke`.

The rewrite's runtime is modern (React 19; antd **6** is a dependency but is used
only for the `App` context provider and `Form` — every visible admin control is
hand-built `.ant-*` DOM styled by the antd **v3** CSS below). So "old framework"
here means *vendored CSS/icon-font look*, not old running JavaScript.

| Filename prefix | Upstream library | Version |
| --- | --- | --- |
| `admin-antd-*` | Ant Design | **3.26.20** (oracle-pinned; the look the rewrite replicates) |
| `admin-bootstrap-*` | Bootstrap | **4.x** (bundled via OneUI; exact patch not recovered from the bundle) |
| `admin-oneui-*`, `admin-animations.css` | OneUI admin template (pixelcave) | as shipped in the packaged frontend (release not precisely pinned) |
| `admin-plugin-widgets.css` | OneUI bundled plugin widgets (simplebar, datepicker, select2, dropzone, CKEditor, flatpickr, slick, jvectormap, dataTables, …) | as shipped in OneUI |
| `admin-icon-fonts.css` + `static/fa-*` | Font Awesome | **5 (Free)** |

Versions are intentionally **not** upgraded: the project goal is a faithful replica
of the frozen oracle, and the oracle's appearance *is* these framework versions.
Upgrading would change the look and regress `make visual-smoke` — that would be a
redesign, not a cleanup. See the repo replica goal in `AGENTS.md`.

## 2. Authored V2Board source styles (source-owned)

Every file whose header begins `/* Authored V2Board …` is V2Board's own styling
(identifiable by `___`-hashed CSS-module classes or `.v2board-*` selectors). These
are the organized source layer and may be edited normally. In this app:

- `admin-runtime-base.css` — admin runtime base + ticket/chat component styles
  (`.tag___`, `.bubble___`, …) and app-level antd overrides.

## How to classify a file here

A style file is **authored V2Board source** iff its header says `Authored V2Board`
(it contains `___` module classes or `.v2board-` selectors). Everything else is
**vendored** third-party CSS restored verbatim from the packaged frontend.
