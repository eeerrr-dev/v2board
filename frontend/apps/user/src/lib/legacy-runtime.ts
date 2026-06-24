import { legacyGetLocale } from '@v2board/i18n';

export type LegacyBrowserEngine = 'chromium' | 'firefox' | 'other' | 'webkit';

export interface LegacyRuntimeProfile {
  browserEngine: LegacyBrowserEngine;
  locale: string;
  narrowViewport: boolean;
  userAgent: string;
  viewportWidth: number | null;
}

export interface LegacyRuntimeProfileOptions {
  locale?: string;
  userAgent?: string;
  viewportWidth?: number | null;
}

export function detectLegacyBrowserEngine(userAgent: string): LegacyBrowserEngine {
  const normalized = userAgent.toLowerCase();
  if (normalized.includes('firefox')) return 'firefox';
  if (/(chrome|chromium|crios|edg)/i.test(userAgent)) return 'chromium';
  if (normalized.includes('applewebkit')) return 'webkit';
  return 'other';
}

export function createLegacyRuntimeProfile(
  options: LegacyRuntimeProfileOptions = {},
): LegacyRuntimeProfile {
  const userAgent =
    options.userAgent ?? (typeof navigator === 'undefined' ? '' : navigator.userAgent);
  let viewportWidth: number | null;
  if ('viewportWidth' in options) {
    viewportWidth = options.viewportWidth ?? null;
  } else {
    viewportWidth = typeof window === 'undefined' ? null : window.innerWidth;
  }

  return {
    browserEngine: detectLegacyBrowserEngine(userAgent),
    locale: options.locale ?? legacyGetLocale(),
    narrowViewport: viewportWidth !== null && viewportWidth < 768,
    userAgent,
    viewportWidth,
  };
}

export function legacyFixedColumnBodyRowHeightOffset(
  profile: LegacyRuntimeProfile = createLegacyRuntimeProfile(),
): number {
  const localeNeedsOffset = profile.locale === 'fa-IR' || profile.locale === 'vi-VN';
  const webkitOrLocalizedNonFirefoxOffset =
    profile.browserEngine !== 'firefox' &&
    (profile.browserEngine === 'webkit' || localeNeedsOffset);
  const firefoxJapaneseDesktopOffset =
    profile.browserEngine === 'firefox' &&
    profile.locale === 'ja-JP' &&
    !profile.narrowViewport;

  return webkitOrLocalizedNonFirefoxOffset || firefoxJapaneseDesktopOffset ? 1 : 0;
}

export function legacyOrdersBodyRowHeightOffset(
  rowCount: number,
  profile: LegacyRuntimeProfile = createLegacyRuntimeProfile(),
): number {
  return rowCount > 2 ? legacyFixedColumnBodyRowHeightOffset(profile) : 0;
}
