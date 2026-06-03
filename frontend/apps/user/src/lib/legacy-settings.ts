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
    g_lang?: string;
    g_langSeparator?: string;
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
    antRadioFocusShadow: string;
    antRadioButtonFocusShadow: string;
    altPrimaryText: string;
    altPrimaryBg: string;
    altPrimaryHoverBg: string;
    altPrimaryFocusShadow: string;
    altPrimaryDisabledText: string;
    altPrimaryActiveText: string;
    altPrimaryActiveBg: string;
    customControlActiveBg: string;
    link: string;
    linkHover: string;
    linkActive: string;
    page: string;
    blockShadow: string;
    blockHeaderBg: string;
    blockPopShadow: string;
    blockPopActiveShadow: string;
    blockShadowHover: string;
    blockShadowActive: string;
    headerDarkColor: string;
    sidebarDarkColor: string;
    sidebarDarkBg: string;
    activeBg: string;
    navHeading: string;
    navLink: string;
    navIcon: string;
    navSubmenuBg: string;
    navSubmenuLink: string;
    navSubmenuLinkHover: string;
    navDarkHeading: string;
    navDarkLink: string;
    navIconDark: string;
    navDarkActiveBg: string;
    navDarkSubmenuBg: string;
    navDarkSubmenuLink: string;
    navDarkHorizontalBg: string;
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
    antRadioFocusShadow: 'rgba(6, 101, 208, 0.08)',
    antRadioButtonFocusShadow: 'rgba(6, 101, 208, 0.06)',
    altPrimaryText: '#054d9e',
    altPrimaryBg: '#cde4fe',
    altPrimaryHoverBg: '#a8d0fc',
    altPrimaryFocusShadow: 'rgba(146, 196, 252, 0.25)',
    altPrimaryDisabledText: '#212529',
    altPrimaryActiveText: '#022954',
    altPrimaryActiveBg: '#92c4fc',
    customControlActiveBg: '#4299fa',
    link: '#0665d0',
    linkHover: '#2a84de',
    linkActive: '#004aab',
    page: '#f0f3f8',
    blockShadow: '0 1px 3px rgba(219, 226, 239, 0.5), 0 1px 2px rgba(219, 226, 239, 0.5)',
    blockHeaderBg: '#f8f9fc',
    blockPopShadow: '#d4dcec',
    blockPopActiveShadow: '#edf0f7',
    blockShadowHover: '#d4dcec',
    blockShadowActive: '#e2e8f2',
    headerDarkColor: '#c8d2e6',
    sidebarDarkColor: '#e4e9f3',
    sidebarDarkBg: '#343a40',
    activeBg: '#e1effe',
    navHeading: '#949da5',
    navLink: '#555d65',
    navIcon: 'rgba(6, 101, 208, 0.7)',
    navSubmenuBg: '#f5faff',
    navSubmenuLink: '#78838e',
    navSubmenuLinkHover: '#383d42',
    navDarkHeading: '#7a8793',
    navDarkLink: '#c0c6cc',
    navIconDark: '#626d78',
    navDarkActiveBg: '#2a2f33',
    navDarkSubmenuBg: '#2d3238',
    navDarkSubmenuLink: '#a4adb5',
    navDarkHorizontalBg: '#0559b7',
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
    antRadioFocusShadow: 'rgba(59, 89, 152, 0.08)',
    antRadioButtonFocusShadow: 'rgba(59, 89, 152, 0.06)',
    altPrimaryText: '#1e2e4f',
    altPrimaryBg: '#bbc8e4',
    altPrimaryHoverBg: '#9fb2da',
    altPrimaryFocusShadow: 'rgba(142, 165, 211, 0.25)',
    altPrimaryDisabledText: '#fff',
    altPrimaryActiveText: '#090e17',
    altPrimaryActiveBg: '#8ea5d3',
    customControlActiveBg: '#718dc8',
    link: '#3b5998',
    linkHover: '#5b75a6',
    linkActive: '#273c73',
    page: '#f5f6fa',
    blockShadow: '0 2px 6px rgba(231, 234, 243, 0.4)',
    blockHeaderBg: '#fcfcfd',
    blockPopShadow: '#dadeec',
    blockPopActiveShadow: '#f2f3f8',
    blockShadowHover: '#dadeec',
    blockShadowActive: '#e7eaf3',
    headerDarkColor: '#ccd1e6',
    sidebarDarkColor: '#e7eaf3',
    sidebarDarkBg: '#35383e',
    activeBg: '#d8e0f0',
    navHeading: '#869099',
    navLink: '#495057',
    navIcon: '#3b5998',
    navSubmenuBg: '#eef1f8',
    navSubmenuLink: 'rgba(73, 80, 87, 0.75)',
    navSubmenuLinkHover: '#000',
    navDarkHeading: '#a3add1',
    navDarkLink: '#e7eaf3',
    navIconDark: '#a8b9dd',
    navDarkActiveBg: '#222428',
    navDarkSubmenuBg: '#2e3136',
    navDarkSubmenuLink: 'rgba(231, 234, 243, 0.75)',
    navDarkHorizontalBg: '#222428',
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
    antRadioFocusShadow: 'rgba(52, 58, 64, 0.08)',
    antRadioButtonFocusShadow: 'rgba(52, 58, 64, 0.06)',
    altPrimaryText: '#060708',
    altPrimaryBg: '#c0c6cc',
    altPrimaryHoverBg: '#abb3bb',
    altPrimaryFocusShadow: 'rgba(159, 168, 177, 0.25)',
    altPrimaryDisabledText: '#fff',
    altPrimaryActiveText: '#000',
    altPrimaryActiveBg: '#9fa8b1',
    customControlActiveBg: '#626d78',
    link: '#343a40',
    linkHover: '#484a4d',
    linkActive: '#13161a',
    page: '#f5f5f5',
    blockShadow: '0 2px 6px rgba(235, 235, 235, 0.4)',
    blockHeaderBg: '#fafafa',
    blockPopShadow: '#e1e1e1',
    blockPopActiveShadow: '#f2f2f2',
    blockShadowHover: '#e1e1e1',
    blockShadowActive: '#ebebeb',
    headerDarkColor: '#d6d6d6',
    sidebarDarkColor: '#ebebeb',
    sidebarDarkBg: '#35393e',
    activeBg: '#e9ecef',
    navHeading: '#869099',
    navLink: '#495057',
    navIcon: '#6d7a86',
    navSubmenuBg: '#f8f9fa',
    navSubmenuLink: 'rgba(73, 80, 87, 0.75)',
    navSubmenuLinkHover: '#000',
    navDarkHeading: '#b8b8b8',
    navDarkLink: '#ebebeb',
    navIconDark: '#b2bac1',
    navDarkActiveBg: '#1d2023',
    navDarkSubmenuBg: '#292c30',
    navDarkSubmenuLink: 'rgba(235, 235, 235, 0.75)',
    navDarkHorizontalBg: '#1d2023',
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
    antRadioFocusShadow: 'rgba(49, 151, 149, 0.08)',
    antRadioButtonFocusShadow: 'rgba(49, 151, 149, 0.06)',
    altPrimaryText: '#287a79',
    altPrimaryBg: '#caeeed',
    altPrimaryHoverBg: '#ade4e3',
    altPrimaryFocusShadow: 'rgba(156, 223, 221, 0.25)',
    altPrimaryDisabledText: '#212529',
    altPrimaryActiveText: '#154040',
    altPrimaryActiveBg: '#9cdfdd',
    customControlActiveBg: '#62ccca',
    link: '#319795',
    linkHover: '#184a49',
    linkActive: '#1e6f70',
    page: '#f5f5f5',
    blockShadow: '0 1px 3px rgba(228, 228, 228, 0.5), 0 1px 2px rgba(228, 228, 228, 0.5)',
    blockHeaderBg: '#fafafa',
    blockPopShadow: '#e1e1e1',
    blockPopActiveShadow: '#f2f2f2',
    blockShadowHover: '#e1e1e1',
    blockShadowActive: '#ebebeb',
    headerDarkColor: '#d6d6d6',
    sidebarDarkColor: '#ebebeb',
    sidebarDarkBg: '#35393e',
    activeBg: '#ebebeb',
    navHeading: '#869099',
    navLink: '#555d65',
    navIcon: 'rgba(49, 151, 149, 0.7)',
    navSubmenuBg: '#f5f5f5',
    navSubmenuLink: '#78838e',
    navSubmenuLinkHover: '#383d42',
    navDarkHeading: '#7d858f',
    navDarkLink: '#c1c5ca',
    navIconDark: '#646c75',
    navDarkActiveBg: '#2a2e32',
    navDarkSubmenuBg: '#2e3136',
    navDarkSubmenuLink: '#a6acb3',
    navDarkHorizontalBg: '#2b8482',
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
  const value = String(text);
  let mark: HTMLSpanElement | null = null;
  let range: Range | null = null;
  const restoreSelection = deselectCurrentSelection();

  try {
    range = document.createRange();
    const selection = document.getSelection();
    mark = document.createElement('span');
    mark.textContent = value;
    mark.ariaHidden = 'true';
    mark.style.all = 'unset';
    mark.style.position = 'fixed';
    mark.style.top = '0';
    mark.style.clip = 'rect(0, 0, 0, 0)';
    mark.style.whiteSpace = 'pre';
    mark.style.webkitUserSelect = 'text';
    (mark.style as CSSStyleDeclaration & { MozUserSelect?: string }).MozUserSelect = 'text';
    (mark.style as CSSStyleDeclaration & { msUserSelect?: string }).msUserSelect = 'text';
    mark.style.userSelect = 'text';
    mark.addEventListener('copy', (event) => {
      event.stopPropagation();
    });

    document.body.appendChild(mark);
    range.selectNodeContents(mark);
    selection?.addRange(range);
    if (!document.execCommand('copy')) throw new Error('copy command was unsuccessful');
  } catch {
    const clipboardData = (window as unknown as {
      clipboardData?: { setData: (format: string, value: string) => void };
    }).clipboardData;
    try {
      if (!clipboardData) throw new Error('clipboardData unavailable');
      clipboardData.setData('text', value);
    } catch {
      const key = /mac os x/i.test(navigator.userAgent) ? '\u2318+C' : 'Ctrl+C';
      window.prompt(`Copy to clipboard: ${key}, Enter`, value);
    }
  } finally {
    const selection = document.getSelection();
    if (selection && range) {
      if (typeof selection.removeRange === 'function') selection.removeRange(range);
      else selection.removeAllRanges();
    }
    if (mark) document.body.removeChild(mark);
    restoreSelection();
  }
}

function deselectCurrentSelection(): () => void {
  const selection = document.getSelection();
  if (!selection?.rangeCount) return () => {};

  const activeElement = document.activeElement;
  const ranges: Range[] = [];
  for (let index = 0; index < selection.rangeCount; index += 1) {
    ranges.push(selection.getRangeAt(index));
  }

  const focusElement =
    activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement
      ? activeElement
      : null;
  focusElement?.blur();
  selection.removeAllRanges();

  return () => {
    if ((selection as Selection & { type?: string }).type === 'Caret') selection.removeAllRanges();
    if (!selection.rangeCount) ranges.forEach((savedRange) => selection.addRange(savedRange));
    focusElement?.focus();
  };
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
  root.style.setProperty('--shadow-block', palette.blockShadow);
  root.style.setProperty('--legacy-block-header-bg', palette.blockHeaderBg);
  root.style.setProperty('--legacy-block-pop-shadow', palette.blockPopShadow);
  root.style.setProperty('--legacy-block-pop-active-shadow', palette.blockPopActiveShadow);
  root.style.setProperty('--legacy-block-shadow-hover', palette.blockShadowHover);
  root.style.setProperty('--legacy-block-shadow-active', palette.blockShadowActive);
  root.style.setProperty('--legacy-header-dark-color', palette.headerDarkColor);
  root.style.setProperty('--legacy-sidebar-dark-color', palette.sidebarDarkColor);
  root.style.setProperty('--legacy-sidebar-dark-bg', palette.sidebarDarkBg);
  root.style.setProperty('--legacy-link', palette.link);
  root.style.setProperty('--legacy-link-hover', palette.linkHover);
  root.style.setProperty('--legacy-link-active', palette.linkActive);
  root.style.setProperty('--legacy-active-bg', palette.activeBg);
  root.style.setProperty('--legacy-nav-heading', palette.navHeading);
  root.style.setProperty('--legacy-nav-link', palette.navLink);
  root.style.setProperty('--legacy-nav-icon', palette.navIcon);
  root.style.setProperty('--legacy-nav-submenu-bg', palette.navSubmenuBg);
  root.style.setProperty('--legacy-nav-submenu-link', palette.navSubmenuLink);
  root.style.setProperty('--legacy-nav-submenu-link-hover', palette.navSubmenuLinkHover);
  root.style.setProperty('--legacy-nav-dark-heading', palette.navDarkHeading);
  root.style.setProperty('--legacy-nav-dark-link', palette.navDarkLink);
  root.style.setProperty('--legacy-nav-icon-dark', palette.navIconDark);
  root.style.setProperty('--legacy-nav-dark-active-bg', palette.navDarkActiveBg);
  root.style.setProperty('--legacy-nav-dark-submenu-bg', palette.navDarkSubmenuBg);
  root.style.setProperty('--legacy-nav-dark-submenu-link', palette.navDarkSubmenuLink);
  root.style.setProperty('--legacy-nav-dark-horizontal-bg', palette.navDarkHorizontalBg);
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
  root.style.setProperty('--legacy-ant-radio-focus-shadow', palette.antRadioFocusShadow);
  root.style.setProperty(
    '--legacy-ant-radio-button-focus-shadow',
    palette.antRadioButtonFocusShadow,
  );
  root.style.setProperty('--legacy-alt-primary-text', palette.altPrimaryText);
  root.style.setProperty('--legacy-alt-primary-bg', palette.altPrimaryBg);
  root.style.setProperty('--legacy-alt-primary-hover-bg', palette.altPrimaryHoverBg);
  root.style.setProperty('--legacy-alt-primary-focus-shadow', palette.altPrimaryFocusShadow);
  root.style.setProperty('--legacy-alt-primary-disabled-text', palette.altPrimaryDisabledText);
  root.style.setProperty('--legacy-alt-primary-active-text', palette.altPrimaryActiveText);
  root.style.setProperty('--legacy-alt-primary-active-bg', palette.altPrimaryActiveBg);
  root.style.setProperty('--legacy-custom-control-active-bg', palette.customControlActiveBg);
  root.style.setProperty('--antd-wave-shadow-color', palette.primary);

  document.title = String(settings.title);
}

function applyLegacyThemeCss(color: LegacyThemeColor, hasHost: boolean): void {
  const href = hasHost
    ? `./theme/${color}.css`
    : `/theme/default/assets/theme/${color}.css`;
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = href;
  document.getElementsByTagName('head')[0]?.appendChild(link);
}
