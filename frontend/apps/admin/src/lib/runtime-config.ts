export interface AdminRuntimeConfig {
  title?: string;
  theme?: {
    color?: string;
  };
  background_url?: string;
  description?: string;
  logo?: string;
  secure_path?: string;
  /** docs/api-dialect.md §10.3: boot-time legacy `#/…` → history-URL toggle. */
  legacy_hash_redirect_enable?: boolean;
}

const DEV_ADMIN_PATH =
  (import.meta.env.VITE_DEV_ADMIN_PATH ?? 'admin').trim().replace(/^\/+|\/+$/g, '') || 'admin';

const DEFAULT_RUNTIME_CONFIG = {
  title: 'V2Board',
  theme: { color: 'default' },
  background_url: '',
  description: 'V2Board',
  logo: '',
  secure_path: DEV_ADMIN_PATH,
  // Mirrors the Rust config default (config.rs legacy_hash_redirect_enable).
  legacy_hash_redirect_enable: true,
} as const satisfies Required<AdminRuntimeConfig>;
const ADMIN_THEME_COLORS = new Set(['black', 'darkblue', 'default', 'green']);
const ADMIN_THEME_META_COLORS: Record<string, string> = {
  default: '#0665d0',
  darkblue: '#3b5998',
  black: '#343a40',
  green: '#319795',
};
const DARK_THEME_META_COLOR = '#171717';

let cachedConfig: AdminRuntimeConfig | undefined;

// The Rust-injected JSON blob is immutable for the page lifetime, so it is
// read, parsed, and validated exactly once; the frozen result is shared.
export function getAdminRuntimeConfig(): AdminRuntimeConfig {
  cachedConfig ??= freezeConfig(readAdminRuntimeConfig());
  return cachedConfig;
}

// Tests swap the injected DOM element between cases; production never does.
export function resetRuntimeConfigForTests(): void {
  cachedConfig = undefined;
}

function freezeConfig(config: AdminRuntimeConfig): AdminRuntimeConfig {
  if (config.theme) Object.freeze(config.theme);
  return Object.freeze(config);
}

function readAdminRuntimeConfig(): AdminRuntimeConfig {
  const element = document.getElementById('v2board-runtime-config');
  const source = element?.textContent?.trim();
  if (!source || source === '__V2BOARD_RUNTIME_CONFIG__') return cloneDefaults();

  try {
    const value: unknown = JSON.parse(source);
    if (!isRecord(value)) return cloneDefaults();
    const theme = isRecord(value.theme) ? value.theme : {};
    return {
      title: stringValue(value.title, DEFAULT_RUNTIME_CONFIG.title),
      theme: { color: stringValue(theme.color, DEFAULT_RUNTIME_CONFIG.theme.color) },
      background_url: stringValue(value.background_url, DEFAULT_RUNTIME_CONFIG.background_url),
      description: stringValue(value.description, DEFAULT_RUNTIME_CONFIG.description),
      logo: stringValue(value.logo, DEFAULT_RUNTIME_CONFIG.logo),
      secure_path: stringValue(value.secure_path, DEFAULT_RUNTIME_CONFIG.secure_path),
      legacy_hash_redirect_enable:
        typeof value.legacy_hash_redirect_enable === 'boolean'
          ? value.legacy_hash_redirect_enable
          : DEFAULT_RUNTIME_CONFIG.legacy_hash_redirect_enable,
    };
  } catch {
    return cloneDefaults();
  }
}

export function applyAdminRuntimeConfig(): void {
  const config = getAdminRuntimeConfig();
  document.title = getAdminTitle();
  const requestedColor = config.theme?.color ?? 'default';
  document.documentElement.dataset.themeColor = ADMIN_THEME_COLORS.has(requestedColor)
    ? requestedColor
    : 'default';
  upsertMeta('description').content = config.description?.trim() || document.title;
  syncAdminThemeColorMeta();
}

export function syncAdminThemeColorMeta(): void {
  const requestedColor = getAdminRuntimeConfig().theme?.color ?? 'default';
  const color = ADMIN_THEME_COLORS.has(requestedColor) ? requestedColor : 'default';
  upsertMeta('theme-color').content = document.documentElement.classList.contains('dark')
    ? DARK_THEME_META_COLOR
    : ADMIN_THEME_META_COLORS[color]!;
}

export function getAdminTitle(): string {
  return getAdminRuntimeConfig().title || DEFAULT_RUNTIME_CONFIG.title;
}

export function getAdminLogo(): string {
  return getOperatorImageUrl(getAdminRuntimeConfig().logo);
}

export function getAdminBackgroundUrl(): string {
  return getOperatorImageUrl(getAdminRuntimeConfig().background_url);
}

export function getAdminApiBaseUrl(): string {
  return `${new URL(window.location.href).origin}/api/v1`;
}

export function getAdminSecurePath(): string | null {
  const securePath = getAdminRuntimeConfig().secure_path;
  if (typeof securePath !== 'string') return null;
  return securePath.replaceAll('/', '');
}

/** The admin router basename (docs/api-dialect.md §10.1): `/{admin_path}`. */
export function getAdminBasename(): string {
  const securePath = getAdminSecurePath();
  return securePath ? `/${securePath}` : '/';
}

export function getLegacyHashRedirectEnabled(): boolean {
  return (
    getAdminRuntimeConfig().legacy_hash_redirect_enable ??
    DEFAULT_RUNTIME_CONFIG.legacy_hash_redirect_enable
  );
}

function cloneDefaults(): AdminRuntimeConfig {
  return { ...DEFAULT_RUNTIME_CONFIG, theme: { ...DEFAULT_RUNTIME_CONFIG.theme } };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === 'string' ? value : fallback;
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

function upsertMeta(name: string): HTMLMetaElement {
  const existing = document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`);
  if (existing) return existing;
  const meta = document.createElement('meta');
  meta.name = name;
  document.head.append(meta);
  return meta;
}
