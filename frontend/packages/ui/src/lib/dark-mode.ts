import { useSyncExternalStore } from 'react';
import { readCookie, writeCookie } from '@v2board/i18n';

const DARK_MODE_KEY = 'dark_mode';

export type ThemePreference = 'system' | 'light' | 'dark';

/**
 * Build one app-local theme store while keeping the shared persistence and
 * browser-subscription behavior in the UI platform. The callback is the only
 * app-specific part: each shell owns its runtime theme-color meta tag.
 */
export function createDarkModeStore(syncThemeColorMeta: () => void) {
  const listeners = new Set<() => void>();

  function readThemePreference(): ThemePreference {
    const raw = readCookie(DARK_MODE_KEY);
    if (raw === '1') return 'dark';
    if (raw === '0') return 'light';
    return 'system';
  }

  function systemPrefersDark(): boolean {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  }

  function resolveDarkMode(preference: ThemePreference = readThemePreference()): boolean {
    if (preference === 'dark') return true;
    if (preference === 'light') return false;
    return systemPrefersDark();
  }

  function isDarkModeApplied(): boolean {
    if (typeof document === 'undefined') return false;
    return document.documentElement.classList.contains('dark');
  }

  function applyDarkMode(enabled = resolveDarkMode()): void {
    document.documentElement.classList.toggle('dark', enabled);
    document.documentElement.style.colorScheme = enabled ? 'dark' : 'light';
    syncThemeColorMeta();
  }

  function handleSystemThemeChange(): void {
    if (readThemePreference() !== 'system') return;
    applyDarkMode(resolveDarkMode('system'));
    listeners.forEach((listener) => listener());
  }

  function applyInitialDarkMode(): void {
    applyDarkMode(resolveDarkMode());
    window
      .matchMedia('(prefers-color-scheme: dark)')
      .addEventListener('change', handleSystemThemeChange);
  }

  function setThemePreference(preference: ThemePreference): void {
    applyDarkMode(resolveDarkMode(preference));
    writeCookie(
      DARK_MODE_KEY,
      preference === 'dark' ? '1' : preference === 'light' ? '0' : 'system',
    );
    listeners.forEach((listener) => listener());
  }

  function subscribeDarkMode(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }

  function useDarkMode(): boolean {
    return useSyncExternalStore(subscribeDarkMode, isDarkModeApplied, isDarkModeApplied);
  }

  function useThemePreference(): ThemePreference {
    return useSyncExternalStore(subscribeDarkMode, readThemePreference, readThemePreference);
  }

  return {
    applyDarkMode,
    applyInitialDarkMode,
    isDarkModeApplied,
    readThemePreference,
    resolveDarkMode,
    setThemePreference,
    subscribeDarkMode,
    systemPrefersDark,
    useDarkMode,
    useThemePreference,
  };
}
