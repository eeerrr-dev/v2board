import { beforeEach, describe, expect, it } from 'vitest';
import { applyInitialDarkMode, isDarkModeEnabled, setDarkMode } from './dark-mode';

describe('dark mode shadcn class storage', () => {
  beforeEach(() => {
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    document.documentElement.style.colorScheme = '';
    window.localStorage.clear();
  });

  it('reads dark mode from the legacy cookie', () => {
    document.cookie = 'dark_mode=1;path=/';
    window.localStorage.setItem('dark_mode', '0');

    expect(isDarkModeEnabled()).toBe(true);
  });

  it('ignores malformed legacy dark mode cookie encoding during startup and keeps light tokens', () => {
    document.cookie = 'dark_mode=%E0%A4%A;path=/';

    expect(isDarkModeEnabled()).toBe(false);
    expect(() => applyInitialDarkMode()).not.toThrow();
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('enables the shadcn class dark theme and writes the legacy cookie', () => {
    setDarkMode(true);

    expect(document.cookie).toContain('dark_mode=1');
    expect(window.localStorage.getItem('dark_mode')).toBeNull();
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('disables the shadcn class dark theme and keeps the original cookie key', () => {
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';

    setDarkMode(false);

    expect(document.cookie).toContain('dark_mode=0');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('syncs startup to light shadcn tokens when the cookie is absent', () => {
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';

    applyInitialDarkMode();

    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('enables shadcn class dark tokens on startup when the cookie is set', () => {
    document.cookie = 'dark_mode=1;path=/';

    applyInitialDarkMode();

    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });
});
