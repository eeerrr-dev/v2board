import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { getCurrentLocale, i18nGet } from './errors';

const originalNavigatorLanguage = window.navigator.language;

describe('legacy error locale lookup', () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.g_lang = undefined;
    window.g_langSeparator = undefined;
    window.settings = undefined;
  });

  afterEach(() => {
    Object.defineProperty(window.navigator, 'language', {
      value: originalNavigatorLanguage,
      configurable: true,
    });
  });

  it('falls back to zh-CN instead of navigator language before the provider stamps g_lang', () => {
    Object.defineProperty(window.navigator, 'language', {
      value: 'en-US',
      configurable: true,
    });

    expect(getCurrentLocale()).toBe('zh-CN');
    expect(i18nGet('请求失败')).toBe('请求失败');
  });

  it('prefers legacy storage and then g_lang', () => {
    window.g_lang = 'ja-JP';
    expect(getCurrentLocale()).toBe('ja-JP');

    window.localStorage.setItem('umi_locale', 'en-US');
    expect(getCurrentLocale()).toBe('en-US');
  });

  it('uses bundled legacy dictionaries for the resolved locale', () => {
    window.g_lang = 'en-US';
    window.settings = {
      i18n: [] as unknown as string[] & Record<string, Record<string, string>>,
    };
    window.settings.i18n!['en-US'] = { 请求失败: 'Legacy failed' };

    expect(i18nGet('请求失败')).toBe('Legacy failed');
  });

  it('uses localized fallback dictionaries when the bundled assets are absent', () => {
    window.g_lang = 'zh-TW';
    expect(i18nGet('请求失败')).toBe('請求失敗');

    window.g_lang = 'vi-VN';
    expect(i18nGet('请求失败')).toBe('Yêu Cầu Thất Bại');

    window.g_lang = 'ko-KR';
    expect(i18nGet('请求失败')).toBe('요청실패');
  });
});
