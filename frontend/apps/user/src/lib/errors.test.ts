import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createI18n } from '@v2board/i18n/testing';
import { getCurrentLocale, i18nGet } from './errors';

const originalNavigatorLanguage = window.navigator.language;

describe('error locale lookup', () => {
  beforeEach(() => {
    createI18n();
    window.localStorage.clear();
  });

  afterEach(() => {
    Object.defineProperty(window.navigator, 'language', {
      value: originalNavigatorLanguage,
      configurable: true,
    });
  });

  it('falls back to zh-CN instead of navigator language before bootstrap writes v2board_locale', () => {
    Object.defineProperty(window.navigator, 'language', {
      value: 'en-US',
      configurable: true,
    });

    expect(getCurrentLocale()).toBe('zh-CN');
    expect(i18nGet('请求失败')).toBe('请求失败');
  });

  it('reads only the canonical v2board_locale key, never legacy keys', () => {
    window.localStorage.setItem('umi_locale', 'ja-JP');
    expect(getCurrentLocale()).toBe('zh-CN');

    window.localStorage.setItem('v2board_locale', 'en-US');
    expect(getCurrentLocale()).toBe('en-US');
  });

  it('uses the complete locale resources for localized errors', () => {
    window.localStorage.setItem('v2board_locale', 'en-US');
    expect(i18nGet('请求失败')).toBe('Request failed');

    window.localStorage.setItem('v2board_locale', 'ja-JP');
    expect(i18nGet('请求失败')).toBe('Request failed');

    window.localStorage.setItem('v2board_locale', 'zh-TW');
    expect(i18nGet('请求失败')).toBe('請求失敗');

    window.localStorage.setItem('v2board_locale', 'vi-VN');
    expect(i18nGet('请求失败')).toBe('Yêu Cầu Thất Bại');

    window.localStorage.setItem('v2board_locale', 'ko-KR');
    expect(i18nGet('请求失败')).toBe('요청실패');
  });
});
