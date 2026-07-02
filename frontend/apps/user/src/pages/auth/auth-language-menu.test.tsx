import { screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { AuthLanguageMenu } from './auth-language-menu';

const i18nMocks = vi.hoisted(() => ({ changeLanguage: vi.fn() }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'common.language' ? 'Language' : key),
    i18n: { language: 'en-US', changeLanguage: i18nMocks.changeLanguage },
  }),
}));

describe('AuthLanguageMenu', () => {
  beforeEach(() => {
    i18nMocks.changeLanguage.mockClear();
    window.localStorage.setItem('umi_locale', 'en-US');
    window.g_lang = 'en-US';
    window.settings = { i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>> };
  });

  afterEach(() => {
    document.cookie = 'i18n=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    window.localStorage.clear();
    window.settings = undefined;
    window.g_lang = undefined;
    vi.restoreAllMocks();
  });

  it('opens a menu of the enabled locales from a native labelled button trigger', async () => {
    const { user } = renderWithProviders(<AuthLanguageMenu />);

    const trigger = screen.getByRole('button', { name: 'Language: English' });
    expect(trigger.tagName).toBe('BUTTON');
    expect(trigger).toHaveAttribute('type', 'button');
    expect(trigger).toHaveTextContent('English');
    // visual-parity.mjs drives the auth switcher through this hook.
    expect(trigger).toHaveClass('v2board-auth-language-trigger');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');

    await user.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const menu = await screen.findByRole('menu');
    // The auth variant renders plain items (LanguageMenuItems passes
    // role={undefined}, which wipes Radix's menuitem role), so select entries
    // through the class hook visual-parity.mjs also drives.
    const items = Array.from(menu.querySelectorAll('.v2board-auth-language-menu-item'));
    expect(items.map((item) => item.textContent)).toEqual(['English', '简体中文']);
  });

  it('persists locale selection in place via changeLanguage without a full-page reload', async () => {
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);
    const { user } = renderWithProviders(<AuthLanguageMenu />);

    await user.click(screen.getByRole('button', { name: 'Language: English' }));
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByText('简体中文'));

    // Persistence writes stay (Tier-1 language persistence contract)...
    expect(document.cookie).toContain('i18n=zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBe('zh-CN');
    // ...but the switch is reactive: changeLanguage drives the re-render, no reload.
    expect(i18nMocks.changeLanguage).toHaveBeenCalledWith('zh-CN');
    expect(reload).not.toHaveBeenCalled();
  });
});
