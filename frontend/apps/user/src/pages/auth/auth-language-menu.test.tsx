import { screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import { setRuntimeConfig } from '@/test/runtime-config';
import { AuthLanguageMenu } from './auth-language-menu';

const i18nMocks = vi.hoisted(() => ({ changeLanguage: vi.fn() }));

vi.mock('react-i18next', () => ({
  useTranslation: () => {
    const translation = createTestTranslation({ 'common.language': 'Language' }, 'en-US');
    return {
      ...translation,
      i18n: { ...translation.i18n, changeLanguage: i18nMocks.changeLanguage },
    };
  },
}));

describe('AuthLanguageMenu', () => {
  beforeEach(() => {
    i18nMocks.changeLanguage.mockClear();
    window.localStorage.setItem('v2board_locale', 'en-US');
    setRuntimeConfig({ i18n: ['en-US', 'zh-CN'] });
  });

  afterEach(() => {
    window.localStorage.clear();
    setRuntimeConfig();
    vi.restoreAllMocks();
  });

  it('opens a menu of the enabled locales from a native labelled button trigger', async () => {
    const { user } = renderWithProviders(<AuthLanguageMenu />);

    const trigger = screen.getByRole('button', { name: 'Language: English' });
    expect(trigger.tagName).toBe('BUTTON');
    expect(trigger).toHaveAttribute('type', 'button');
    expect(trigger).toHaveTextContent('English');
    expect(trigger).toHaveAttribute('data-testid', 'auth-language-trigger');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');

    await user.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const menu = await screen.findByRole('menu');
    const items = Array.from(menu.querySelectorAll('[data-slot="dropdown-menu-radio-item"]'));
    expect(items.map((item) => item.textContent)).toEqual(['English', '简体中文']);
    expect(within(menu).getByRole('menuitemradio', { name: 'English' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('persists locale selection in place via changeLanguage without a full-page reload', async () => {
    const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => undefined);
    const { user } = renderWithProviders(<AuthLanguageMenu />);

    await user.click(screen.getByRole('button', { name: 'Language: English' }));
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByText('简体中文'));

    // Persistence stays (Tier-1 language persistence contract) on the canonical
    // §11 key; legacy keys (i18n cookie, umi_locale) are never written again...
    expect(window.localStorage.getItem('v2board_locale')).toBe('zh-CN');
    expect(document.cookie).not.toContain('i18n=zh-CN');
    expect(window.localStorage.getItem('umi_locale')).toBeNull();
    // ...but the switch is reactive: changeLanguage drives the re-render, no reload.
    expect(i18nMocks.changeLanguage).toHaveBeenCalledWith('zh-CN');
    expect(reload).not.toHaveBeenCalled();
  });
});
