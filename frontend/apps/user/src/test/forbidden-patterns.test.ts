import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// ─────────────────────────────────────────────────────────────────────────────
// Central forbidden-pattern guard for the pure-shadcn user island.
//
// Per-page test files own BEHAVIOR (rendered DOM, user events, payloads,
// routing). This file owns the anti-regression PATTERN bans that used to live
// as scattered readFileSync / rendered-HTML `not.toContain` pins in those
// files. Every rule cites the per-page test file the guard migrated from.
//
// Scoping matters: several banned identifiers are legitimate elsewhere —
// `useSyncExternalStore` lives in lib/dark-mode.ts and components/ui/
// confirm-dialog.tsx, `refetchInterval` is owned by lib/queries.ts and passed
// by pages/tickets/detail.tsx, `window.location.pathname` builds the invite
// copy link, `md:grid-cols-2` lays out plan cards, profile.tsx composes the
// shared TableRow primitive for its static deposit table, and the auth LAYOUT
// (not the card) hosts AuthPanelBrand/AuthLanguageMenu. Each rule therefore
// applies to the narrowest file set that reproduces the original guard.
// ─────────────────────────────────────────────────────────────────────────────

const srcDir = join(dirname(fileURLToPath(import.meta.url)), '..');

interface SourceFile {
  /** Posix-style path relative to src/, e.g. 'pages/orders/detail.tsx'. */
  rel: string;
  text: string;
}

// Non-test runtime source only: *.test.* files and src/test helpers are the
// guard's own tooling, not shipped code.
function collectSources(dir: string, out: SourceFile[]): SourceFile[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const abs = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== 'test') collectSources(abs, out);
      continue;
    }
    if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) continue;
    out.push({ rel: relative(srcDir, abs).split(sep).join('/'), text: readFileSync(abs, 'utf8') });
  }
  return out;
}

const sources = collectSources(srcDir, []);

interface Rule {
  /** Carried into the failure message so a hit explains itself. */
  why: string;
  /** Tested per line; keep patterns non-global. */
  pattern: RegExp;
  /** Exact src-relative paths, or directory prefixes ending in '/'. Default: every source. */
  scope?: string[];
  /** Sanctioned exception: return true to permit a matching line. */
  allow?: (line: string, rel: string) => boolean;
}

function inScope(rel: string, scope?: string[]): boolean {
  if (!scope) return true;
  return scope.some((s) => (s.endsWith('/') ? rel.startsWith(s) : rel === s));
}

function violations(rules: Rule[]): string[] {
  const hits: string[] = [];
  for (const rule of rules) {
    for (const { rel, text } of sources) {
      if (!inScope(rel, rule.scope)) continue;
      text.split('\n').forEach((line, index) => {
        if (!rule.pattern.test(line)) return;
        if (rule.allow?.(line, rel)) return;
        hits.push(`${rel}:${index + 1} ${line.trim()} — ${rule.why}`);
      });
    }
  }
  return hits;
}

// ── Import graph ─────────────────────────────────────────────────────────────

const IMPORT_SPECIFIER_PATTERNS = [
  /\bfrom\s*['"]([^'"]+)['"]/g, // static import + re-export
  /\bimport\s+['"]([^'"]+)['"]/g, // side-effect import
  /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g, // dynamic import
];

interface ImportEdge {
  rel: string;
  line: number;
  /** '@/'-alias and relative specifiers resolved to a src-relative module id. */
  resolved: string;
  specifier: string;
}

function resolveSpecifier(rel: string, specifier: string): string {
  if (specifier.startsWith('@/')) return specifier.slice(2);
  if (specifier.startsWith('.')) return join(dirname(rel), specifier).split(sep).join('/');
  return specifier; // package specifier
}

function collectImportEdges(): ImportEdge[] {
  const edges: ImportEdge[] = [];
  for (const { rel, text } of sources) {
    for (const pattern of IMPORT_SPECIFIER_PATTERNS) {
      for (const match of text.matchAll(pattern)) {
        const specifier = match[1];
        if (specifier === undefined) continue;
        edges.push({
          line: text.slice(0, match.index).split('\n').length,
          rel,
          resolved: resolveSpecifier(rel, specifier),
          specifier,
        });
      }
    }
  }
  return edges;
}

const importEdges = collectImportEdges();

// The auth ACTION CARD: brand chrome and the language menu are hosted by
// pages/auth/auth-layout.tsx outside the card and must not creep back inside.
const AUTH_CARD_FILES = [
  'pages/auth/login.tsx',
  'pages/auth/register.tsx',
  'pages/auth/forget.tsx',
  'pages/auth/auth-panel.tsx',
  'pages/auth/auth-fields.tsx',
  'pages/auth/auth-tos-field.tsx',
];

describe('forbidden module imports', () => {
  it('never restores or imports retired legacy modules anywhere in src', () => {
    // from: orders/detail.test.tsx (retired '@/components/ui/shadcn-dialog'
    // migration name; the canonical registry path is '@/components/ui/dialog'),
    // register/forget tests (lib/legacy-toast), login tests
    // (retired components/layout/auth-language-menu), toast.test.ts
    // (@radix-ui/react-toast store replaced by Sonner), form-field.test.tsx
    // (retired cloneElement FormField), and form.test.tsx (retired
    // FormProvider/context stack). RHF forms now compose Controller directly
    // with the shared '@/components/ui/field' primitive.
    const banned = new Map<string, string>([
      [
        'components/ui/shadcn-dialog',
        "retired migration name — use the canonical '@/components/ui/dialog' registry path",
      ],
      [
        'components/ui/form',
        "retired FormProvider stack — use Controller with '@/components/ui/field'",
      ],
      [
        'components/ui/form-field',
        "retired cloneElement FormField — use Controller with '@/components/ui/field'",
      ],
      ['lib/legacy-toast', "retired legacy toast — use '@/lib/toast'"],
      ['components/layout/auth-language-menu', 'retired auth language-menu module'],
      ['@radix-ui/react-toast', 'legacy self-owned toast store — Sonner owns toasts'],
    ]);
    const retiredSourceModules = new Set([
      'components/ui/form.tsx',
      'components/ui/form-field.tsx',
      'components/ui/shadcn-dialog.tsx',
    ]);
    const restoredModules = sources
      .filter(({ rel }) => retiredSourceModules.has(rel))
      .map(({ rel }) => rel);
    const hits = importEdges
      .filter((edge) => banned.has(edge.resolved))
      .map(
        (edge) =>
          `${edge.rel}:${edge.line} imports '${edge.specifier}' — ${banned.get(edge.resolved)}`,
      );
    expect(restoredModules).toEqual([]);
    expect(hits).toEqual([]);
  });

  it('keeps brand chrome and the language menu out of the auth action card', () => {
    // from: login.test.tsx / register.test.tsx / forget.test.tsx — the card
    // files must not import the (legitimate, layout-hosted) brand or
    // language-menu modules.
    const banned = new Set([
      'pages/auth/auth-language-menu',
      'components/layout/language-menu',
      'pages/auth/auth-brand',
    ]);
    const hits = importEdges
      .filter((edge) => AUTH_CARD_FILES.includes(edge.rel) && banned.has(edge.resolved))
      .map((edge) => `${edge.rel}:${edge.line} imports '${edge.specifier}'`);
    expect(hits).toEqual([]);
  });
});

// ── Legacy presentation class names (src-wide) ───────────────────────────────

describe('legacy class names never appear in source', () => {
  it('bans Ant Design / antd-mobile / legacy icon and carousel classes', () => {
    // from: orders/index, tickets/index, invite, traffic, node, knowledge,
    // dashboard, table, toast, confirm-dialog, auth-recaptcha and
    // auth-language-menu tests (ant-table-*, ant-badge-status, ant-spin-*,
    // ant-empty, ant-drawer, ant-modal, ant-dropdown, ant-message,
    // ant-notification, ant-btn-*, ant-tag, am-list, anticon-loading,
    // slick-slider, fa fa-plus, si si-check).
    expect(
      violations([
        { pattern: /\bant-[a-z]/, why: 'Ant Design class names are banned on redesigned surfaces' },
        { pattern: /\banticon/, why: 'Ant Design icon classes are banned' },
        { pattern: /\bam-list/, why: 'antd-mobile list classes are banned' },
        { pattern: /\bslick-slider\b/, why: 'legacy slick carousel foundation is banned' },
        {
          pattern: /\bfa fa-/,
          why: 'Font Awesome legacy icon classes are banned — use lucide-react',
        },
        { pattern: /\bsi si-/, why: 'Simple Line Icons classes are banned — use lucide-react' },
      ]),
    ).toEqual([]);
  });

  it('bans Bootstrap / OneUI chrome classes', () => {
    // from: login, register, forget, dashboard, plans/index, checkout,
    // orders/detail, invite, tickets/index, traffic, guest-layout and
    // app-layout tests (block block-rounded, block block-link-pop, block-title,
    // block-mode-loading, form-control, btn btn-block/btn-primary,
    // bg-gray-lighter, list-group-item, nav-main-link, content content-full,
    // font-size-sm text-uppercase text-muted).
    expect(
      violations([
        {
          pattern: /\bblock block-|\bblock-title\b|\bblock-mode-loading\b/,
          why: 'OneUI block card/loading classes are banned',
        },
        {
          pattern: /\bform-control\b/,
          why: 'Bootstrap form-control chrome is banned',
        },
        {
          pattern: /\bbtn-block\b|\bbtn-primary\b|className="btn[\s"]/,
          why: 'Bootstrap button classes are banned',
        },
        {
          pattern: /(["'`])(?:[^"'`]*\s)?block(?:\s[^"'`]*)?\1/,
          why: 'bare `block` class token is banned — OneUI owns .block',
        },
        { pattern: /\bbg-gray-lighter\b/, why: 'OneUI bg-gray-lighter utility is banned' },
        { pattern: /\blist-group/, why: 'Bootstrap list-group foundation is banned' },
        { pattern: /\bnav-main-link\b/, why: 'OneUI sidebar nav class is banned' },
        { pattern: /content content-full/, why: 'OneUI content wrapper class is banned' },
        { pattern: /\bfont-size-sm\b/, why: 'Bootstrap typography utility stack is banned' },
      ]),
    ).toEqual([]);
  });

  it('bans packaged-bundle CSS-module hashes and packaged theme asset paths', () => {
    // from: tickets/detail.test.tsx (content___DW5w1 / input___1j_ND /
    // tag___12_9H) and dashboard.test.tsx (oneClickSubscribe___2t9Xg,
    // /theme/default/assets/ and copied source-owned client icon directories).
    expect(
      violations([
        {
          pattern: /[A-Za-z]___[A-Za-z0-9_-]{4}/,
          why: 'packaged-bundle CSS-module class hashes are banned',
        },
        {
          pattern: /\/theme\/default\/assets\//,
          why: 'packaged legacy theme asset paths are banned',
        },
        {
          pattern: /assets\/images\/icon\//,
          scope: ['pages/dashboard-subscribe-menu.tsx'],
          why: 'all subscribe targets use one Lucide Import glyph, not copied client artwork',
        },
      ]),
    ).toEqual([]);
  });

  it('bans the tw: gradual-reskin prefix — the user app is a pure shadcn island', () => {
    // from: login.test.tsx and guest-layout.test.tsx; AGENTS.md explicitly
    // keeps this app-wide tw:-absence guard.
    expect(
      violations([{ pattern: /\btw:/, why: 'tw:-prefixed utilities are banned in the user app' }]),
    ).toEqual([]);
  });

  it('bans hardcoded Chinese placeholder literals — placeholders go through i18n', () => {
    // from: register.test.tsx (placeholder="请输入密码"). i18nGet('…') Chinese
    // KEYS are legitimate; a raw Chinese placeholder attribute is not.
    expect(
      violations([
        {
          pattern: /placeholder="[^"]*[一-鿿]/,
          why: 'hardcoded Chinese placeholder — use t()/i18nGet',
        },
      ]),
    ).toEqual([]);
  });
});

// ── Retired auth-surface chrome ──────────────────────────────────────────────

describe('retired auth chrome never returns', () => {
  it('bans replica-era auth shell hooks anywhere in src', () => {
    // from: guest-layout.test.tsx, login.test.tsx, register.test.tsx,
    // forget.test.tsx. Note: .v2board-auth-box remains a legitimate ORACLE-side
    // readySelector in the interaction-parity harness (frontend/tests/lib) — the ban is only on
    // user-app sources.
    expect(
      violations([
        {
          pattern:
            /v2board-auth-visual|v2board-auth-backdrop|v2board-auth-box|v2board-background\b|v2board-auth-chrome/,
          why: 'replica-era auth chrome class is retired',
        },
      ]),
    ).toEqual([]);
  });

  it('bans the split-visual two-column layout on the auth island', () => {
    // from: login.test.tsx / register.test.tsx (md:grid-cols-2 wrapper). The
    // plans grid uses md:grid-cols-2 legitimately, so this is auth-scoped.
    expect(
      violations([
        {
          pattern: /md:grid-cols-2/,
          scope: ['pages/auth/', 'components/layout/guest-layout.tsx'],
          why: 'retired split-visual auth layout',
        },
      ]),
    ).toEqual([]);
  });

  it('keeps packaged theme logo paths out of the auth action card', () => {
    expect(
      violations([
        {
          pattern: /\/theme\/logo\.png/,
          scope: AUTH_CARD_FILES,
          why: 'operator assets come from validated runtime config',
        },
      ]),
    ).toEqual([]);
  });

  it('does not interpolate operator image URLs into inline auth CSS', () => {
    // guest-layout.test.tsx pins the safe <img src> customization path.
    expect(
      violations([
        {
          pattern: /backgroundImage|style=\{\{[^}]*background/,
          scope: ['components/layout/guest-layout.tsx', 'pages/auth/auth-layout.tsx'],
          why: 'operator image URLs belong in src attributes, not CSS strings',
        },
      ]),
    ).toEqual([]);
  });

  it('bans hardcoded hash anchors throughout auth — use react-router Link', () => {
    // React Router owns every internal auth transition while createHashRouter
    // keeps the externally visible #/login, #/register and #/forgetpassword URLs.
    expect(
      violations([
        {
          pattern: /href="#\//,
          scope: ['components/layout/guest-layout.tsx', 'pages/auth/'],
          why: 'in-app links must route through react-router Link',
        },
      ]),
    ).toEqual([]);
  });
});

// ── Retired legacy identifiers (src-wide, all zero-legitimate-use) ──────────

describe('retired legacy identifiers never return', () => {
  it('bans retired helpers, shims and wrappers anywhere in src', () => {
    // from: orders/detail (LegacyLoadingIcon), confirm-dialog (legacyConfirm),
    // register/forget (useLegacyRecaptcha, readFormValue), api (isLegacyTimeoutError,
    // LEGACY_AUTH_STORAGE_KEY), App (LegacyUnknownRouteRedirect, USER_ROUTE_ELEMENTS,
    // routeComponentKey, KeyedAppLayout, KeyedGuestLayout), knowledge
    // (lockLegacyDrawerBodyScroll, AntBtn), traffic (useFixedColumnRowHeights,
    // bodyRowHeightOffset, fixedBodyRowExtraPixel), sanitize-html (support-probe
    // family), Stripe legacy token/CardElement flow, login/tickets-detail (keyCode).
    expect(
      violations([
        { pattern: /\bLegacyLoadingIcon\b/, why: 'legacy loading icon is retired' },
        { pattern: /\blegacyConfirm\b/, why: 'legacy confirm implementation is retired' },
        { pattern: /\buseLegacyRecaptcha\b/, why: 'legacy recaptcha hook is retired' },
        { pattern: /\breadFormValue\b/, why: 'retired DOM form-reader helper' },
        {
          pattern: /\bisLegacyTimeoutError\b/,
          why: 'timeout special-casing is retired — all transport failures stay silent',
        },
        {
          pattern: /\bLEGACY_AUTH_STORAGE_KEY\b/,
          why: 'raw legacy auth key is owned by lib/auth.ts helpers',
        },
        {
          pattern: /\bLegacyUnknownRouteRedirect\b/,
          why: 'render-time unknown-route redirect is retired — catch-all loader owns it',
        },
        {
          pattern: /\bUSER_ROUTE_ELEMENTS\b/,
          why: 'eager route-element map is retired — routes are lazy modules',
        },
        {
          pattern: /\brouteComponentKey\b/,
          why: 'per-route layout keying caused remounts on sibling navigation',
        },
        {
          pattern: /\bKeyedAppLayout\b|\bKeyedGuestLayout\b/,
          why: 'keyed layout wrappers are retired',
        },
        {
          pattern: /lockLegacyDrawerBodyScroll/,
          why: 'legacy body-scroll drawer wiring is retired — Radix Sheet owns it',
        },
        { pattern: /\bAntBtn\b/, why: 'Ant Design button foundation is banned' },
        {
          pattern:
            /\buseFixedColumnRowHeights\b|\bbodyRowHeightOffset\b|\bfixedBodyRowExtraPixel\b/,
          why: 'legacy fixed-column row-height shim is retired',
        },
        {
          pattern:
            /DOMPurify\.isSupported|\bdomPurifySupported\b|\bisDOMPurifySupported\b|\bisDOMPurifyReliable\b|\bcanSanitizeWithDOMPurify\b/,
          why: 'DOMPurify support probes are retired — DOMPurify is the only sanitizer',
        },
        {
          pattern: /\bsanitizeLegacyHtmlWithDomApi\b/,
          why: 'hand-rolled DOM-API sanitizer is retired',
        },
        {
          pattern: /\bcreateToken\b|\bCardElement\b|stripe-card-form/,
          why: 'Stripe Payment Element + PaymentIntent replaced legacy card tokens',
        },
        {
          pattern: /\bkeyCode\b/,
          why: 'deprecated keyCode key handling is banned — use native form submit / event.key',
        },
      ]),
    ).toEqual([]);
  });
});

// ── Scoped architecture and API bans ─────────────────────────────────────────

describe('scoped legacy APIs and patterns', () => {
  it('App.tsx stays on the data router without route-level Suspense', () => {
    // from: App.test.tsx. Both bans are App.tsx-scoped: other components may
    // legitimately use <Suspense>, and route modules define their own trees.
    expect(
      violations([
        {
          pattern: /<Routes[\s>/]/,
          scope: ['App.tsx'],
          why: 'component-router <Routes> is banned — createHashRouter owns routing',
        },
        {
          pattern: /<Suspense\b/,
          scope: ['App.tsx'],
          why: 'route-level Suspense is banned — hydrateFallbackElement + lazy route modules own pending UI',
        },
      ]),
    ).toEqual([]);
  });

  it('lib/api.ts permanently tears down an unauthorized session without credential restoration', () => {
    // from: api.test.ts. Reading window.location.href (URL parsing) stays
    // legal; assignment/replace/pathname interpolation and any direct
    // localStorage access and delayed credential restoration are not.
    expect(
      violations([
        {
          pattern: /\bsetAuthData\s*\(/,
          scope: ['lib/api.ts'],
          why: 'the 403 handler must not restore or mutate credentials directly',
        },
        {
          pattern: /\bsetTimeout\s*\(/,
          scope: ['lib/api.ts'],
          why: 'delayed credential restoration is banned',
        },
        {
          pattern: /\blocalStorage\b/,
          scope: ['lib/api.ts'],
          why: 'storage access must go through lib/auth helpers',
        },
        {
          pattern: /window\.location\.href\s*=/,
          scope: ['lib/api.ts'],
          why: 'full-page navigation is banned — park only the hash on #/login',
        },
        {
          pattern: /window\.location\.replace\s*\(/,
          scope: ['lib/api.ts'],
          why: 'history-replacing full-page redirect is banned',
        },
        {
          pattern: /window\.location\.pathname/,
          scope: ['lib/api.ts'],
          why: 'pathname-interpolated login URLs are banned',
        },
        {
          pattern: /\/timeout\/i/,
          scope: ['lib/api.ts'],
          why: 'timeout message sniffing is banned — all transport failures stay silent',
        },
      ]),
    ).toEqual([]);
  });

  it('orders/detail.tsx stays free of antd Modal / legacy qrcode props and owns no poll cadence', () => {
    // from: orders/detail.test.tsx. All bans are file-scoped: footer= is a
    // legitimate AuthPanel prop, key={index} is legitimate in plan-content and
    // tickets/detail, and refetchInterval is owned by lib/queries.ts (this
    // page may only toggle `enabled`).
    const scope = ['pages/orders/detail.tsx'];
    expect(
      violations([
        {
          pattern: /\bclosable|maskClosable|width=\{300\}|\bcentered\b|\bfooter=/,
          scope,
          why: 'Ant Design v3 Modal prop residue is banned on the QR dialog',
        },
        {
          pattern: /\brenderAs\b|\bincludeMargin\b|\bbgColor\b|\bfgColor\b|\blevel=/,
          scope,
          why: 'legacy qrcode.react v1 props are banned — QRCodeSVG uses value/size',
        },
        {
          pattern: /key=\{index\}/,
          scope,
          why: 'payment-method list identity must be key={method.id}',
        },
        {
          pattern: /refetchInterval/,
          scope,
          why: 'poll cadence is owned by useOrderStatus in lib/queries',
        },
      ]),
    ).toEqual([]);
  });

  it('pages keep controlled form state instead of imperative refs / FormData reads', () => {
    // from: login, register, forget, checkout, profile and tickets/detail
    // tests (formRef/emailRef/passwordRef/confirmPasswordRef/emailCodeRef/
    // oldPasswordRef/newPasswordRef/giftCardRef/couponRef/inputRef,
    // .current!.value reads, new FormData).
    const scope = ['pages/'];
    expect(
      violations([
        {
          pattern: /new FormData\s*\(/,
          scope,
          why: 'FormData form plumbing is banned — use controlled/schema form state',
        },
        {
          pattern:
            /\b(?:form|email|password|confirmPassword|emailCode|oldPassword|newPassword|giftCard|coupon|input)Ref\b/,
          scope,
          why: 'imperative input refs for form values are banned',
        },
        { pattern: /\.current[!?]?\.value/, scope, why: 'imperative ref value reads are banned' },
        {
          // from: login.test.tsx — the legacy global Enter-key listener; submit
          // flows through native form submission. Scoped to pages/auth/ because
          // shell components may own real keydown shortcuts.
          pattern: /addEventListener\(\s*['"]keydown['"]/,
          scope: ['pages/auth/'],
          why: 'global keydown submit listeners are banned — use native form submit',
        },
      ]),
    ).toEqual([]);
  });

  it('pages never hand-roll portals, native option pickers, or window markdown hooks', () => {
    // from: knowledge/index.test.tsx (createPortal, window.copy/window.jump)
    // and register.test.tsx (<option suffix picker — the Radix combobox is
    // behavior-covered in register.test.tsx).
    const scope = ['pages/'];
    expect(
      violations([
        {
          pattern: /\bcreatePortal\b/,
          scope,
          why: 'hand-rolled portals are banned — Radix primitives own portaling',
        },
        {
          pattern: /<option\b/,
          scope,
          why: 'native <option> pickers are banned — use the Radix combobox/select',
        },
        {
          pattern: /window\.(copy|jump)\b/,
          scope,
          why: 'global window markdown hooks are banned — use data-v2board-markdown-action delegation',
        },
      ]),
    ).toEqual([]);
  });

  it('redesigned tables compose the shared DataTable, not page-local rows', () => {
    // from: invite.test.tsx, traffic.test.tsx, node.test.tsx. profile.tsx is
    // deliberately outside this scope: its static deposit table composes the
    // shared TableRow primitive directly, which its own tests cover.
    expect(
      violations([
        {
          pattern: /<TableRow|<TableCell/,
          scope: ['pages/invite.tsx', 'pages/traffic.tsx', 'pages/node.tsx'],
          why: 'rows must render through the shared DataTable primitive',
        },
      ]),
    ).toEqual([]);
  });

  it('file-scoped bans: profile form state, ticket cache/polling, recaptcha dialog, confirm-dialog, toaster', () => {
    expect(
      violations([
        // from: profile.test.tsx — schema-based form state, not per-keystroke useState.
        {
          pattern: /\bsetPasswordForm\b|\bsetGiftCard\b/,
          scope: ['pages/profile.tsx'],
          why: 'per-keystroke useState form tracking is banned',
        },
        // from: tickets/index.test.tsx — list refresh is owned by the mutations' onSuccess.
        {
          pattern: /\bremoveQueries\b/,
          scope: ['pages/tickets/index.tsx'],
          why: 'page-level ticket cache cleanup is banned',
        },
        // from: tickets/detail.test.tsx — polling flows through refetchInterval
        // (the { refetchInterval: 5000 } option is behavior-asserted there).
        {
          pattern: /\bsetTimeout\b/,
          scope: ['pages/tickets/detail.tsx'],
          why: 'hand-rolled polling timers are banned — React Query refetchInterval owns polling',
        },
        // from: auth-recaptcha.test.tsx — no legacy dialog-bridge closable prop.
        {
          pattern: /\bclosable/,
          scope: ['pages/auth/auth-recaptcha.tsx'],
          why: 'legacy dialog-bridge closable prop is banned',
        },
        {
          pattern: /\bsetTimeout\b|delayCaptchaIframeRemoving/,
          scope: ['pages/auth/auth-recaptcha.tsx'],
          why: 'solved reCAPTCHA actions run immediately; delayed legacy iframe/token shims are banned',
        },
        // from: confirm-dialog.test.ts — no Ant modal ActionButton compatibility code.
        {
          pattern: /\bActionButton\b/,
          scope: ['components/ui/confirm-dialog.tsx'],
          why: 'Ant modal ActionButton compatibility is banned',
        },
        // from: toast.test.ts — Sonner owns toasts; both bans are toaster-scoped
        // (dark-mode.ts/confirm-dialog.tsx legitimately use useSyncExternalStore,
        // and sanitized markdown containers legitimately use innerHTML).
        {
          pattern: /\buseSyncExternalStore\b/,
          scope: ['components/ui/toaster.tsx'],
          why: 'hand-rolled toast store subscription is banned',
        },
        {
          pattern: /innerHTML/,
          scope: ['components/ui/toaster.tsx'],
          why: 'innerHTML toast rendering is banned',
        },
      ]),
    ).toEqual([]);
  });
});

// ── Storage-key ownership ────────────────────────────────────────────────────

describe('storage-key ownership', () => {
  it("only lib/auth.ts touches the 'authorization' storage key", () => {
    // from: api.test.ts. The session-key contract stays centralized in the
    // lib/auth helpers; nothing else may spell the raw key.
    expect(
      violations([
        {
          allow: (_line, rel) => rel === 'lib/auth.ts',
          pattern: /['"]authorization['"]/i,
          why: "the raw 'authorization' storage key is owned by lib/auth.ts",
        },
      ]),
    ).toEqual([]);
  });
});
