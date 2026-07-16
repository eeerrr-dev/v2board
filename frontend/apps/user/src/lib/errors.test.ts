import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createI18n } from '@v2board/i18n/testing';
import { getCurrentLocale, i18nGet } from './errors';

const originalNavigatorLanguage = window.navigator.language;

describe('error locale lookup', () => {
  beforeEach(() => {
    createI18n();
    window.localStorage.clear();
    window.g_lang = undefined;
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

  it('prefers persisted storage and then g_lang', () => {
    window.g_lang = 'ja-JP';
    expect(getCurrentLocale()).toBe('ja-JP');

    window.localStorage.setItem('umi_locale', 'en-US');
    expect(getCurrentLocale()).toBe('en-US');
  });

  it('uses the complete locale resources for localized errors', () => {
    window.g_lang = 'en-US';
    expect(i18nGet('请求失败')).toBe('Request failed');

    window.g_lang = 'ja-JP';
    expect(i18nGet('请求失败')).toBe('Request failed');

    window.g_lang = 'zh-TW';
    expect(i18nGet('请求失败')).toBe('請求失敗');

    window.g_lang = 'vi-VN';
    expect(i18nGet('请求失败')).toBe('Yêu Cầu Thất Bại');

    window.g_lang = 'ko-KR';
    expect(i18nGet('请求失败')).toBe('요청실패');
  });
});
