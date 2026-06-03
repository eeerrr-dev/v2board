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
  const link = document.createElement('link');
  const color = settings.theme?.color;
  link.rel = 'stylesheet';
  link.href = settings.host ? `./theme/${color}.css` : `/assets/admin/theme/${color}.css`;
  document.getElementsByTagName('head')[0]?.appendChild(link);
}
