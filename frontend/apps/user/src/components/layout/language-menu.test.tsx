import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LanguageMenu } from './language-menu';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('LanguageMenu antd dropdown behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    window.localStorage.setItem('umi_locale', 'en-US');
    window.g_lang = 'en-US';
    window.settings = { i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>> };
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    window.localStorage.clear();
    window.settings = undefined;
    window.g_lang = undefined;
    vi.useRealTimers();
  });

  it('removes slide-down enter classes after the rc-animate enter motion ends', () => {
    act(() => {
      root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-login-i18n-btn" />);
    });

    const trigger = container.querySelector('.v2board-login-i18n-btn') as HTMLElement;
    trigger.getBoundingClientRect = () =>
      ({
        top: 50,
        right: 90,
        bottom: 70,
        left: 30,
        width: 60,
        height: 20,
        x: 30,
        y: 50,
        toJSON: () => {},
      }) as DOMRect;

    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const menu = document.body.querySelector('.ant-dropdown-menu') as HTMLElement;
    expect(menu.className).toContain('slide-down-enter');
    expect(menu.className).not.toContain('slide-down-enter-active');

    act(() => {
      vi.advanceTimersByTime(30);
    });

    expect(menu.className).toContain('slide-down-enter-active');

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(menu.className).not.toContain('slide-down-enter');
    expect(menu.className).not.toContain('slide-down-enter-active');
  });

  it('sorts only the enabled locale array like legacy SelectLang', () => {
    const legacyI18n = [
      'zh-CN',
      'en-US',
      'ja-JP',
      'vi-VN',
      'ko-KR',
      'zh-TW',
      'fa-IR',
    ] as string[] & Record<string, Record<string, string>>;
    legacyI18n['zh-CN'] = { 请求失败: '请求失败' };
    legacyI18n['ko-KR'] = { 请求失败: '요청실패' };
    window.settings = { i18n: legacyI18n };

    act(() => {
      root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-login-i18n-btn" />);
    });

    const trigger = container.querySelector('.v2board-login-i18n-btn') as HTMLElement;
    trigger.getBoundingClientRect = () =>
      ({
        top: 50,
        right: 90,
        bottom: 70,
        left: 30,
        width: 60,
        height: 20,
        x: 30,
        y: 50,
        toJSON: () => {},
      }) as DOMRect;

    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(
      [...document.body.querySelectorAll('.ant-dropdown-menu-item')].map((item) => item.textContent),
    ).toEqual(['English', 'فارسی', '日本語', '한국어', 'Tiếng Việt', '简体中文', '繁體中文']);
  });

  it('keeps the old unkeyed locale menu item map from SelectLang', () => {
    const source = readFileSync('src/components/layout/language-menu.tsx', 'utf8');
    const menuSource = source.slice(
      source.indexOf('{locales.map((locale) => ('),
      source.indexOf('</ul>', source.indexOf('{locales.map((locale) => (')),
    );

    expect(menuSource).toContain('{locales.map((locale) => (');
    expect(menuSource).not.toContain('key={locale.code}');
    expect(menuSource).not.toContain('key=');
  });

  it('keeps the legacy antd Dropdown base CSS', () => {
    const css = readFileSync('src/styles/globals.css', 'utf8');

    expect(css).toContain('.ant-dropdown {\n  box-sizing: border-box;\n  position: absolute;');
    expect(css).toContain('z-index: 1050;\n  display: block;');
    expect(css).toContain('-webkit-transform: translateZ(0);');
  });
});
