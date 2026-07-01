import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AuthLanguageMenu } from './auth-language-menu';

const i18nMocks = vi.hoisted(() => ({ changeLanguage: vi.fn() }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'common.language' ? 'Language' : key),
    i18n: { language: 'en-US', changeLanguage: i18nMocks.changeLanguage },
  }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('AuthLanguageMenu', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    i18nMocks.changeLanguage.mockClear();
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
    document.cookie = 'i18n=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    window.localStorage.clear();
    window.settings = undefined;
    window.g_lang = undefined;
    vi.restoreAllMocks();
  });

  it('renders the redesigned auth trigger as a native button with a Radix menu', () => {
    act(() => {
      root.render(<AuthLanguageMenu />);
    });

    const trigger = container.querySelector('.v2board-auth-language-trigger') as HTMLElement;
    expect(trigger.tagName).toBe('BUTTON');
    expect(trigger.getAttribute('type')).toBe('button');
    expect(trigger.textContent).toBe('English');
    expect(trigger.getAttribute('aria-label')).toBe('Language: English');
    expect(trigger.hasAttribute('role')).toBe(false);
    expect(trigger.hasAttribute('tabindex')).toBe(false);
    expect(trigger.getAttribute('aria-haspopup')).toBe('menu');
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(trigger.querySelectorAll('svg')).toHaveLength(1);
    expect(trigger.className).toContain('h-9');
    expect(trigger.className).toContain('font-medium');
    expect(trigger.className).not.toContain('hover:bg-accent');
    expect(trigger.className).not.toContain('border');
    expect(container.querySelectorAll('button')).toHaveLength(1);

    act(() => {
      trigger.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
    });
    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(document.body.querySelector('.ant-dropdown-menu')).toBeNull();
    expect(document.body.querySelector('.v2board-auth-language-menu-content')).not.toBeNull();
    expect(
      [...document.body.querySelectorAll('.v2board-auth-language-menu-item')].map(
        (item) => item.textContent,
      ),
    ).toEqual(['English', '简体中文']);
  });

  it('persists locale selection in place via changeLanguage without a full-page reload', () => {
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);
    act(() => {
      root.render(<AuthLanguageMenu />);
    });

    const trigger = container.querySelector('.v2board-auth-language-trigger') as HTMLElement;
    act(() => {
      trigger.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
    });

    const zhCN = [...document.body.querySelectorAll('.v2board-auth-language-menu-item')].find(
      (item) => item.textContent === '简体中文',
    ) as HTMLElement;
    act(() => {
      zhCN.dispatchEvent(new Event('click', { bubbles: true }));
    });

    // Persistence writes stay (Tier-1 language persistence contract)...
    expect(document.cookie).toContain('i18n=zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    // ...but the switch is now reactive: changeLanguage drives the re-render, no reload.
    expect(i18nMocks.changeLanguage).toHaveBeenCalledWith('zh-CN');
    expect(reload).not.toHaveBeenCalled();
    expect(document.body.querySelector('.ant-dropdown')).toBeNull();
  });
});
