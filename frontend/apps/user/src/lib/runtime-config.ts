import { SUPPORTED_LOCALES } from '@v2board/i18n';

export type RuntimeThemeColor = 'default' | 'darkblue' | 'black' | 'green';

/**
 * docs/api-dialect.md §10.6: the typed chat-widget object Rust injects into the
 * user runtime config when — and only when — a provider is completely
 * configured. Absent means the feature is off.
 */
export type ChatWidgetConfig =
  | { provider: 'crisp'; website_id: string }
  | { provider: 'tawk'; property_id: string; widget_id: string };

export interface RuntimeConfig {
  title?: string;
  theme?: {
    color?: RuntimeThemeColor;
  };
  background_url?: string;
  description?: string;
  logo?: string;
  i18n?: string[];
  /** docs/api-dialect.md §10.3: boot-time legacy `#/…` → history-URL toggle. */
  legacy_hash_redirect_enable?: boolean;
  /** docs/api-dialect.md §10.6: absent = chat widget off (the default). */
  chat_widget?: ChatWidgetConfig;
}

const DEFAULT_RUNTIME_CONFIG = {
  title: 'V2Board',
  theme: { color: 'default' },
  background_url: '',
  description: 'V2Board',
  logo: '',
  // The bundled locale registry is the authority for which locales exist; the
  // Rust-injected list is intersected with it and kept set-equal by
  // `make deploy-contract-audit`.
  i18n: SUPPORTED_LOCALES.map((locale) => locale.code),
  // Mirrors the Rust config default (config.rs legacy_hash_redirect_enable).
  legacy_hash_redirect_enable: true,
  // `chat_widget` is deliberately absent: off is the §10.6 default.
} as const satisfies Required<Omit<RuntimeConfig, 'chat_widget'>>;
const THEME_COLORS = new Set<RuntimeThemeColor>(['default', 'darkblue', 'black', 'green']);
const THEME_META_COLORS: Record<RuntimeThemeColor, string> = {
  default: '#0665d0',
  darkblue: '#3b5998',
  black: '#343a40',
  green: '#319795',
};
const DARK_THEME_META_COLOR = '#171717';

let cachedConfig: RuntimeConfig | undefined;

// The Rust-injected JSON blob is immutable for the page lifetime, so it is
// read, parsed, and validated exactly once; the frozen result is shared.
export function getRuntimeConfig(): RuntimeConfig {
  cachedConfig ??= freezeConfig(readRuntimeConfig());
  return cachedConfig;
}

// Tests swap the injected DOM element between cases; production never does.
export function resetRuntimeConfigForTests(): void {
  cachedConfig = undefined;
}

function freezeConfig(config: RuntimeConfig): RuntimeConfig {
  if (config.theme) Object.freeze(config.theme);
  if (config.i18n) Object.freeze(config.i18n);
  if (config.chat_widget) Object.freeze(config.chat_widget);
  return Object.freeze(config);
}

function readRuntimeConfig(): RuntimeConfig {
  const element = document.getElementById('v2board-runtime-config');
  const source = element?.textContent?.trim();
  if (!source || source === '__V2BOARD_RUNTIME_CONFIG__') return cloneDefaults();

  try {
    const value: unknown = JSON.parse(source);
    if (!isRecord(value)) return cloneDefaults();
    const theme = isRecord(value.theme) ? value.theme : {};
    return {
      title: stringValue(value.title, DEFAULT_RUNTIME_CONFIG.title),
      theme: {
        color: stringValue(theme.color, DEFAULT_RUNTIME_CONFIG.theme.color) as RuntimeThemeColor,
      },
      background_url: stringValue(value.background_url, DEFAULT_RUNTIME_CONFIG.background_url),
      description: stringValue(value.description, DEFAULT_RUNTIME_CONFIG.description),
      logo: stringValue(value.logo, DEFAULT_RUNTIME_CONFIG.logo),
      i18n: Array.isArray(value.i18n)
        ? value.i18n.filter((locale): locale is string => typeof locale === 'string')
        : [...DEFAULT_RUNTIME_CONFIG.i18n],
      legacy_hash_redirect_enable:
        typeof value.legacy_hash_redirect_enable === 'boolean'
          ? value.legacy_hash_redirect_enable
          : DEFAULT_RUNTIME_CONFIG.legacy_hash_redirect_enable,
      chat_widget: parseChatWidget(value.chat_widget),
    };
  } catch {
    return cloneDefaults();
  }
}

// docs/api-dialect.md §10.6: only the two complete typed shapes activate the
// widget; anything partial or unknown stays inert (feature off).
function parseChatWidget(value: unknown): ChatWidgetConfig | undefined {
  if (!isRecord(value)) return undefined;
  if (value.provider === 'crisp' && nonEmptyString(value.website_id)) {
    return { provider: 'crisp', website_id: value.website_id };
  }
  if (
    value.provider === 'tawk' &&
    nonEmptyString(value.property_id) &&
    nonEmptyString(value.widget_id)
  ) {
    return { provider: 'tawk', property_id: value.property_id, widget_id: value.widget_id };
  }
  return undefined;
}

function nonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim() !== '';
}

export function getSiteTitle(): string {
  return getRuntimeConfig().title || DEFAULT_RUNTIME_CONFIG.title;
}

export function getLegacyHashRedirectEnabled(): boolean {
  return (
    getRuntimeConfig().legacy_hash_redirect_enable ??
    DEFAULT_RUNTIME_CONFIG.legacy_hash_redirect_enable
  );
}

/** docs/api-dialect.md §10.6: undefined = chat widget off (the default). */
export function getChatWidgetConfig(): ChatWidgetConfig | undefined {
  return getRuntimeConfig().chat_widget;
}

function getOperatorImageUrl(value: string | undefined): string {
  const candidate = value?.trim();
  if (!candidate) return '';
  try {
    const url = new URL(candidate, window.location.origin);
    return url.protocol === 'http:' || url.protocol === 'https:' ? candidate : '';
  } catch {
    return '';
  }
}

export function getLogoUrl(): string {
  return getOperatorImageUrl(getRuntimeConfig().logo);
}

export function getBackgroundUrl(): string {
  return getOperatorImageUrl(getRuntimeConfig().background_url);
}

export function applyRuntimeConfig(): void {
  const root = document.documentElement;
  const settings = getRuntimeConfig();
  const requestedColor = settings.theme?.color ?? 'default';
  const color = THEME_COLORS.has(requestedColor) ? requestedColor : 'default';
  root.dataset.themeColor = color;
  document.title = getSiteTitle();
  upsertMeta('description').content = settings.description?.trim() || document.title;
  syncRuntimeThemeColorMeta();
}

export function syncRuntimeThemeColorMeta(): void {
  const requestedColor = getRuntimeConfig().theme?.color ?? 'default';
  const color = THEME_COLORS.has(requestedColor) ? requestedColor : 'default';
  upsertMeta('theme-color').content = document.documentElement.classList.contains('dark')
    ? DARK_THEME_META_COLOR
    : THEME_META_COLORS[color];
}

function cloneDefaults(): RuntimeConfig {
  return {
    ...DEFAULT_RUNTIME_CONFIG,
    theme: { ...DEFAULT_RUNTIME_CONFIG.theme },
    i18n: [...DEFAULT_RUNTIME_CONFIG.i18n],
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === 'string' ? value : fallback;
}

function upsertMeta(name: string): HTMLMetaElement {
  const existing = document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`);
  if (existing) return existing;
  const meta = document.createElement('meta');
  meta.name = name;
  document.head.append(meta);
  return meta;
}
