import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LanguageMenu } from './language-menu';
import { readUserStyles } from '../../test/read-user-styles';

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
      root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-legacy-auth-language-trigger" />);
    });

    const trigger = container.querySelector('.v2board-legacy-auth-language-trigger') as HTMLElement;
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

  it('sorts the enabled locale array and drops unsupported locales like legacy SelectLang', () => {
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
      root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-legacy-auth-language-trigger" />);
    });

    const trigger = container.querySelector('.v2board-legacy-auth-language-trigger') as HTMLElement;
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
    ).toEqual(['English', '日本語', '한국어', 'Tiếng Việt', '简体中文', '繁體中文']);
  });

  it('persists the i18n cookie before triggering the legacy locale reload', () => {
    document.cookie = 'i18n=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);

    act(() => {
      root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-legacy-auth-language-trigger" />);
    });

    const trigger = container.querySelector('.v2board-legacy-auth-language-trigger') as HTMLElement;
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

    const zhCN = [...document.body.querySelectorAll('.ant-dropdown-menu-item')].find(
      (item) => item.textContent === '简体中文',
    ) as HTMLElement;
    act(() => {
      zhCN.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });

    expect(document.cookie).toContain('i18n=zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    expect(reload).toHaveBeenCalledOnce();
  });

  it('keeps the top-center dropdown inside the viewport like rc-align overflow adjustment', () => {
    const heightDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
    const widthDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
      configurable: true,
      get() {
        return String(this.className).includes('ant-dropdown') ? 96 : 0;
      },
    });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      configurable: true,
      get() {
        return String(this.className).includes('ant-dropdown') ? 160 : 0;
      },
    });

    try {
      act(() => {
        root.render(<LanguageMenu legacyIcon showLabel triggerClassName="v2board-legacy-auth-language-trigger" />);
      });

      const trigger = container.querySelector('.v2board-legacy-auth-language-trigger') as HTMLElement;
      trigger.getBoundingClientRect = () =>
        ({
          top: 8,
          right: 60,
          bottom: 28,
          left: 20,
          width: 40,
          height: 20,
          x: 20,
          y: 8,
          toJSON: () => {},
        }) as DOMRect;

      act(() => {
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      });

      const popup = document.body.querySelector('.ant-dropdown') as HTMLElement;
      expect(popup.className).toBe('ant-dropdown ant-dropdown-placement-topCenter');
      expect(popup.style.position).toBe('absolute');
      expect(popup.style.top).toBe('0px');
      expect(popup.style.left).toBe('0px');
      expect(popup.style.transform).toBe('');
    } finally {
      if (heightDescriptor) {
        Object.defineProperty(HTMLElement.prototype, 'offsetHeight', heightDescriptor);
      }
      if (widthDescriptor) {
        Object.defineProperty(HTMLElement.prototype, 'offsetWidth', widthDescriptor);
      }
    }
  });

  it('defaults the compact header language trigger to the legacy bottom-center dropdown', () => {
    const heightDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
    const widthDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
      configurable: true,
      get() {
        return String(this.className).includes('ant-dropdown') ? 96 : 0;
      },
    });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      configurable: true,
      get() {
        return String(this.className).includes('ant-dropdown') ? 160 : 0;
      },
    });

    try {
      act(() => {
        root.render(<LanguageMenu legacyIcon triggerClassName="btn btn-primary mr-1" />);
      });

      const trigger = container.querySelector('.btn') as HTMLElement;
      trigger.getBoundingClientRect = () =>
        ({
          top: 8,
          right: 140,
          bottom: 28,
          left: 100,
          width: 40,
          height: 20,
          x: 100,
          y: 8,
          toJSON: () => {},
        }) as DOMRect;

      act(() => {
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      });

      const popup = document.body.querySelector('.ant-dropdown') as HTMLElement;
      expect(popup.className).toBe('ant-dropdown ant-dropdown-placement-bottomCenter');
      expect(popup.style.position).toBe('absolute');
      expect(popup.style.top).toBe('32px');
      expect(popup.style.left).toBe('40px');
      expect(popup.style.transform).toBe('');
    } finally {
      if (heightDescriptor) {
        Object.defineProperty(HTMLElement.prototype, 'offsetHeight', heightDescriptor);
      }
      if (widthDescriptor) {
        Object.defineProperty(HTMLElement.prototype, 'offsetWidth', widthDescriptor);
      }
    }
  });

  it('keys locale menu items by locale code while keeping SelectLang DOM stable', () => {
    const source = readFileSync('src/components/layout/language-menu.tsx', 'utf8');
    const menuSource = source.slice(
      source.indexOf('{locales.map((locale) => ('),
      source.indexOf('</ul>', source.indexOf('{locales.map((locale) => (')),
    );

    expect(menuSource).toContain('{locales.map((locale) => (');
    expect(menuSource).toContain('key={locale.code}');
    expect(menuSource).not.toContain('key={index}');
  });

  it('keeps the legacy antd Dropdown base CSS', () => {
    const css = readUserStyles();

    expect(css).toContain('.ant-dropdown {\n  box-sizing: border-box;\n  position: absolute;');
    expect(css).toContain('z-index: 1050;\n  display: block;');
    expect(css).toContain('-webkit-transform: translateZ(0);');
  });
});
