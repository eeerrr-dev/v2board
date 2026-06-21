export interface AdminLegacySettings {
  title?: string;
  theme?: {
    sidebar?: string;
    header?: string;
    color?: string;
  };
  version?: string;
  host?: string;
  background_url?: string;
  logo?: string;
  secure_path?: string;
}

declare global {
  interface Window {
    settings?: AdminLegacySettings;
  }
}

const ADMIN_THEME_COLORS = new Set(['black', 'darkblue', 'default', 'green']);
const ADMIN_THEME_BASE_HREF = import.meta.env.PROD ? '/assets/admin/themes' : '/src/styles/themes';

export function getAdminSettings(): AdminLegacySettings {
  return window.settings ?? {};
}

export function applyAdminLegacySettings(): void {
  const settings = window.settings;
  if (!settings) return;
  if (typeof settings.secure_path === 'string') {
    settings.secure_path = settings.secure_path.replace('/', '');
  }
  document.title = String(settings.title);
  applyAdminThemeCss(settings);
}

export function getAdminTitle(): string {
  return getAdminSettings().title || 'V2Board';
}

export function getAdminLogo(): string {
  return getAdminSettings().logo || '';
}

export function getAdminBackgroundUrl(): string {
  return getAdminSettings().background_url || '';
}

export function getAdminApiBaseUrl(): string {
  let host = new URL(window.location.href).origin;
  if (getAdminSettings().host) host = getAdminSettings().host!;
  return `${host}/api/v1`;
}

export function getAdminSecurePath(): string | null {
  const securePath = getAdminSettings().secure_path;
  if (typeof securePath !== 'string') return null;
  return securePath.replace('/', '');
}

function applyAdminThemeCss(settings: AdminLegacySettings): void {
  const requestedColor = settings.theme?.color ?? 'default';
  const color = ADMIN_THEME_COLORS.has(requestedColor) ? requestedColor : 'default';
  let link = document.querySelector<HTMLLinkElement>('link[data-v2board-admin-theme-color]');
  if (!link) {
    link = document.createElement('link');
    link.rel = 'stylesheet';
    document.getElementsByTagName('head')[0]?.appendChild(link);
  }
  link.setAttribute('data-v2board-admin-theme-color', color);
  link.href = `${ADMIN_THEME_BASE_HREF}/${color}.css`;
}
