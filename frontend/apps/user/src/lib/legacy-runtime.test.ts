import { afterEach, describe, expect, it } from 'vitest';
import {
  createLegacyRuntimeProfile,
  detectLegacyBrowserEngine,
  legacyFixedColumnBodyRowHeightOffset,
  legacyOrdersBodyRowHeightOffset,
  type LegacyRuntimeProfile,
} from './legacy-runtime';

const CHROME_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';
const FIREFOX_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:126.0) Gecko/20100101 Firefox/126.0';
const SAFARI_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15';

function profile(
  overrides: Partial<LegacyRuntimeProfile> = {},
): LegacyRuntimeProfile {
  const base = createLegacyRuntimeProfile({
    locale: 'zh-CN',
    userAgent: CHROME_UA,
    viewportWidth: 1024,
  });
  return { ...base, ...overrides };
}

describe('legacy runtime profile', () => {
  afterEach(() => {
    window.localStorage.removeItem('umi_locale');
  });

  it('classifies browser engines behind one tested adapter', () => {
    expect(detectLegacyBrowserEngine(FIREFOX_UA)).toBe('firefox');
    expect(detectLegacyBrowserEngine(SAFARI_UA)).toBe('webkit');
    expect(detectLegacyBrowserEngine(CHROME_UA)).toBe('chromium');
    expect(detectLegacyBrowserEngine('curl/8.0.1')).toBe('other');
  });

  it('reads the current locale only through the runtime profile', () => {
    window.localStorage.setItem('umi_locale', 'fa-IR');

    expect(
      createLegacyRuntimeProfile({
        userAgent: CHROME_UA,
        viewportWidth: 1024,
      }),
    ).toMatchObject({
      browserEngine: 'chromium',
      locale: 'fa-IR',
      narrowViewport: false,
      viewportWidth: 1024,
    });
  });

  it('keeps the localized fixed-column row compensation as policy instead of page code', () => {
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'webkit', locale: 'en-US' }),
      ),
    ).toBe(1);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'chromium', locale: 'fa-IR' }),
      ),
    ).toBe(1);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'chromium', locale: 'vi-VN' }),
      ),
    ).toBe(1);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'firefox', locale: 'ja-JP', narrowViewport: false }),
      ),
    ).toBe(1);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'firefox', locale: 'ja-JP', narrowViewport: true }),
      ),
    ).toBe(0);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'firefox', locale: 'fa-IR' }),
      ),
    ).toBe(0);
    expect(
      legacyFixedColumnBodyRowHeightOffset(
        profile({ browserEngine: 'chromium', locale: 'en-US' }),
      ),
    ).toBe(0);
  });

  it('applies the order-table offset only to the long-table shape that needs it', () => {
    const webkitProfile = profile({ browserEngine: 'webkit', locale: 'en-US' });

    expect(legacyOrdersBodyRowHeightOffset(2, webkitProfile)).toBe(0);
    expect(legacyOrdersBodyRowHeightOffset(3, webkitProfile)).toBe(1);
  });
});
