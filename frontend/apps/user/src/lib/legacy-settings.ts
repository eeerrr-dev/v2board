export type LegacyThemeColor = 'default' | 'darkblue' | 'black' | 'green';
export type LegacyTone = 'light' | 'dark';

export interface LegacySettings {
  title?: string;
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
  }
}

// Only the five custom properties read by the shipped user stylesheets remain:
// `--color-brand-500` (user-browser-modes.css), `--color-page`
// (user-document-root.css), and `--legacy-link{,-hover,-active}`
// (user-link-elements.css). Every other legacy theme variable was consumed
// solely by the Bootstrap/OneUI framework CSS that no longer ships, so the
// full per-theme palette table was dropped along with it.
const THEME_COLORS: Record<
  LegacyThemeColor,
  {
    primary: string;
    page: string;
    link: string;
    linkHover: string;
    linkActive: string;
  }
> = {
  default: {
    primary: '#0665d0',
    page: '#f0f3f8',
    link: '#0665d0',
    linkHover: '#2a84de',
    linkActive: '#004aab',
  },
  darkblue: {
    primary: '#3b5998',
    page: '#f5f6fa',
    link: '#3b5998',
    linkHover: '#5b75a6',
    linkActive: '#273c73',
  },
  black: {
    primary: '#343a40',
    page: '#f5f5f5',
    link: '#343a40',
    linkHover: '#484a4d',
    linkActive: '#13161a',
  },
  green: {
    primary: '#319795',
    page: '#f5f5f5',
    link: '#319795',
    linkHover: '#184a49',
    linkActive: '#1e6f70',
  },
};

export function getLegacySettings(): LegacySettings {
  return window.settings ?? {};
}

export function getLegacyTitle(): string {
  return getLegacySettings().title || 'V2Board';
}

export async function copyText(text: string | number | null | undefined): Promise<boolean> {
  const value = String(text);
  if (!navigator.clipboard?.writeText) return copyTextWithExecCommand(value);

  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    return copyTextWithExecCommand(value);
  }
}

function copyTextWithExecCommand(text: string): boolean {
  if (typeof document.execCommand !== 'function') return false;

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'fixed';
  textarea.style.left = '-9999px';
  textarea.style.opacity = '0';
  textarea.style.pointerEvents = 'none';

  document.body.appendChild(textarea);
  textarea.select();
  textarea.setSelectionRange(0, text.length);

  try {
    return document.execCommand('copy');
  } catch {
    return false;
  } finally {
    textarea.remove();
  }
}

export function applyLegacySettings(): void {
  const root = document.documentElement;
  const settings = getLegacySettings();
  const color = settings.theme?.color ?? 'default';
  const palette = THEME_COLORS[color] ?? THEME_COLORS.default;

  // These match the static defaults declared in user-theme-colors.css and
  // user-theme-legacy-tokens.css, so the override is a no-op for the default
  // theme and only repaints the brand/page/link colors for operator-selected
  // non-default palettes. No other legacy variable is read by a shipped
  // stylesheet, so nothing else needs to be set at runtime.
  root.style.setProperty('--color-brand-500', palette.primary);
  root.style.setProperty('--color-page', palette.page);
  root.style.setProperty('--legacy-link', palette.link);
  root.style.setProperty('--legacy-link-hover', palette.linkHover);
  root.style.setProperty('--legacy-link-active', palette.linkActive);

  document.title = String(settings.title);
}
