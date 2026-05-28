export type LegacyThemeColor = 'default' | 'darkblue' | 'black' | 'green';
export type LegacyTone = 'light' | 'dark';

export interface LegacySettings {
  title?: string;
  assets_path?: string;
  theme?: {
    sidebar?: LegacyTone;
    header?: LegacyTone;
    color?: LegacyThemeColor;
  };
  version?: string;
  host?: string;
  background_url?: string;
  description?: string;
  homepage?: string;
  logo?: string;
  i18n?: string[] & Record<string, Record<string, string>>;
}

declare global {
  interface Window {
    settings?: LegacySettings;
  }
}

const THEME_COLORS: Record<
  LegacyThemeColor,
  {
    primary: string;
    primaryHover: string;
    primaryHoverBorder: string;
    primaryActive: string;
    primaryActiveBorder: string;
    primaryFocusShadow: string;
    formFocusBorder: string;
    formFocusShadow: string;
    antHover: string;
    antActive: string;
    antFocusShadow: string;
    altPrimaryText: string;
    altPrimaryBg: string;
    altPrimaryHoverBg: string;
    link: string;
    linkHover: string;
    linkActive: string;
    page: string;
    activeBg: string;
    navIcon: string;
    navIconDark: string;
  }
> = {
  default: {
    primary: '#0665d0',
    primaryHover: '#0553ab',
    primaryHoverBorder: '#054d9e',
    primaryActive: '#054d9e',
    primaryActiveBorder: '#044792',
    primaryFocusShadow: 'rgba(43, 124, 215, 0.5)',
    formFocusBorder: '#5ba6fa',
    formFocusShadow: 'rgba(6, 101, 208, 0.25)',
    antHover: '#2a84de',
    antActive: '#004aab',
    antFocusShadow: 'rgba(6, 101, 208, 0.2)',
    altPrimaryText: '#054d9e',
    altPrimaryBg: '#cde4fe',
    altPrimaryHoverBg: '#a8d0fc',
    link: '#0665d0',
    linkHover: '#2a84de',
    linkActive: '#004aab',
    page: '#f0f3f8',
    activeBg: '#e1effe',
    navIcon: 'rgba(6, 101, 208, 0.7)',
    navIconDark: '#626d78',
  },
  darkblue: {
    primary: '#3b5998',
    primaryHover: '#30497c',
    primaryHoverBorder: '#2d4373',
    primaryActive: '#2d4373',
    primaryActiveBorder: '#293e6a',
    primaryFocusShadow: 'rgba(88, 114, 167, 0.5)',
    formFocusBorder: '#839ccf',
    formFocusShadow: 'rgba(59, 89, 152, 0.25)',
    antHover: '#5b75a6',
    antActive: '#273c73',
    antFocusShadow: 'rgba(59, 89, 152, 0.2)',
    altPrimaryText: '#1e2e4f',
    altPrimaryBg: '#bbc8e4',
    altPrimaryHoverBg: '#9fb2da',
    link: '#3b5998',
    linkHover: '#1e2e4f',
    linkActive: '#273c73',
    page: '#f5f6fa',
    activeBg: '#d8e0f0',
    navIcon: '#3b5998',
    navIconDark: '#a8b9dd',
  },
  black: {
    primary: '#343a40',
    primaryHover: '#23272b',
    primaryHoverBorder: '#1d2124',
    primaryActive: '#1d2124',
    primaryActiveBorder: '#171a1d',
    primaryFocusShadow: 'rgba(82, 88, 93, 0.5)',
    formFocusBorder: '#6d7a86',
    formFocusShadow: 'rgba(52, 58, 64, 0.25)',
    antHover: '#484a4d',
    antActive: '#13161a',
    antFocusShadow: 'rgba(52, 58, 64, 0.2)',
    altPrimaryText: '#060708',
    altPrimaryBg: '#c0c6cc',
    altPrimaryHoverBg: '#abb3bb',
    link: '#0665d0',
    linkHover: '#03356d',
    linkActive: '#13161a',
    page: '#f5f5f5',
    activeBg: '#e9ecef',
    navIcon: '#6d7a86',
    navIconDark: '#b2bac1',
  },
  green: {
    primary: '#319795',
    primaryHover: '#287a79',
    primaryHoverBorder: '#25706f',
    primaryActive: '#25706f',
    primaryActiveBorder: '#216766',
    primaryFocusShadow: 'rgba(80, 167, 165, 0.5)',
    formFocusBorder: '#3dbebb',
    formFocusShadow: 'rgba(49, 151, 149, 0.25)',
    antHover: '#4ea39f',
    antActive: '#1e6f70',
    antFocusShadow: 'rgba(49, 151, 149, 0.2)',
    altPrimaryText: '#287a79',
    altPrimaryBg: '#caeeed',
    altPrimaryHoverBg: '#ade4e3',
    link: '#319795',
    linkHover: '#184a49',
    linkActive: '#1e6f70',
    page: '#f5f5f5',
    activeBg: '#ebebeb',
    navIcon: 'rgba(49, 151, 149, 0.7)',
    navIconDark: '#646c75',
  },
};

export function getLegacySettings(): LegacySettings {
  return window.settings ?? {};
}

export function getLegacyTitle(): string {
  return getLegacySettings().title || 'V2Board';
}

export function getLegacyLogo(): string | null {
  return getLegacySettings().logo || null;
}

export function getLegacyDescription(): string | null {
  return getLegacySettings().description || null;
}

export function isLegacyMobile(): boolean {
  const userAgent = window.navigator.userAgent.toLowerCase();
  return userAgent.includes('mobile');
}

export function legacyCopyText(text: string | undefined): void {
  const input = document.createElement('textarea');
  input.value = String(text);
  input.setAttribute('readonly', '');
  input.style.position = 'fixed';
  input.style.top = '0';
  input.style.left = '0';
  input.style.opacity = '0';
  document.body.appendChild(input);
  input.select();
  document.execCommand('copy');
  document.body.removeChild(input);
}

export function getLegacyTheme() {
  const theme = getLegacySettings().theme ?? {};
  const color = theme.color ?? 'default';
  return {
    sidebar: theme.sidebar ?? 'light',
    header: theme.header ?? 'dark',
    color,
    palette: THEME_COLORS[color] ?? THEME_COLORS.default,
  };
}

export function applyLegacySettings(): void {
  const root = document.documentElement;
  const settings = getLegacySettings();
  const { color, palette } = getLegacyTheme();

  applyLegacyThemeCss(color, Boolean(settings.host));
  root.style.setProperty('--color-brand-400', palette.primaryHover);
  root.style.setProperty('--color-brand-500', palette.primary);
  root.style.setProperty('--color-brand-600', palette.primaryActive);
  root.style.setProperty('--color-page', palette.page);
  root.style.setProperty('--legacy-link', palette.link);
  root.style.setProperty('--legacy-link-hover', palette.linkHover);
  root.style.setProperty('--legacy-link-active', palette.linkActive);
  root.style.setProperty('--legacy-active-bg', palette.activeBg);
  root.style.setProperty('--legacy-nav-icon', palette.navIcon);
  root.style.setProperty('--legacy-nav-icon-dark', palette.navIconDark);
  root.style.setProperty('--legacy-primary-hover', palette.primaryHover);
  root.style.setProperty('--legacy-primary-hover-border', palette.primaryHoverBorder);
  root.style.setProperty('--legacy-primary-active', palette.primaryActive);
  root.style.setProperty('--legacy-primary-active-border', palette.primaryActiveBorder);
  root.style.setProperty('--legacy-primary-focus-shadow', palette.primaryFocusShadow);
  root.style.setProperty('--legacy-form-focus-border', palette.formFocusBorder);
  root.style.setProperty('--legacy-form-focus-shadow', palette.formFocusShadow);
  root.style.setProperty('--legacy-ant-primary', palette.primary);
  root.style.setProperty('--legacy-ant-hover', palette.antHover);
  root.style.setProperty('--legacy-ant-active', palette.antActive);
  root.style.setProperty('--legacy-ant-focus-shadow', palette.antFocusShadow);
  root.style.setProperty('--legacy-alt-primary-text', palette.altPrimaryText);
  root.style.setProperty('--legacy-alt-primary-bg', palette.altPrimaryBg);
  root.style.setProperty('--legacy-alt-primary-hover-bg', palette.altPrimaryHoverBg);
  root.style.setProperty('--antd-wave-shadow-color', palette.primary);

  document.title = String(settings.title);
  const metaTheme = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  metaTheme?.setAttribute('content', palette.primary);
}

function applyLegacyThemeCss(color: LegacyThemeColor, hasHost: boolean): void {
  const href = hasHost
    ? `./theme/${color}.css`
    : `/theme/default/assets/theme/${color}.css`;
  let link = document.querySelector<HTMLLinkElement>('link[data-v2board-theme-color]');
  if (!link) {
    link = document.createElement('link');
    link.rel = 'stylesheet';
    link.dataset.v2boardThemeColor = 'true';
    document.getElementsByTagName('head')[0]?.appendChild(link);
  }
  link.href = href;
}
