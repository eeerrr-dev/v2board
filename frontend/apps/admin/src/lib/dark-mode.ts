import { useSyncExternalStore } from 'react';
import { getLegacyCookie, setLegacyCookie } from './legacy-cookie';

const DARK_MODE_KEY = 'dark_mode';
const listeners = new Set<() => void>();

export type ThemePreference = 'system' | 'light' | 'dark';

// The persisted preference. The frontend-only `dark_mode` cookie stays the single
// storage key: 'dark' → '1' and 'light' → '0' keep the legacy binary values, and
// 'system' (or an absent/unknown cookie) means "follow the OS". Reading tolerates
// both the words and the legacy 1/0 so existing users, the store, and the
// pre-paint script in index.html / dashboard.blade.php all agree.
export function readThemePreference(): ThemePreference {
  const raw = getLegacyCookie(DARK_MODE_KEY);
  if (raw === '1' || raw === 'dark') return 'dark';
  if (raw === '0' || raw === 'light') return 'light';
  return 'system';
}

// Whether the OS currently asks for a dark UI. Guarded so SSR / a jsdom without
// matchMedia degrade to light instead of throwing.
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

// The concrete theme for a preference: an explicit choice wins, 'system' defers
// to the OS.
export function resolveDarkMode(preference: ThemePreference = readThemePreference()): boolean {
  if (preference === 'dark') return true;
  if (preference === 'light') return false;
  return systemPrefersDark();
}

// The applied theme = whether the `.dark` class is really on <html>. Subscribers
// (the trigger icon, the toaster) read this, so they always reflect what is on
// screen and can never get stuck on a stale cookie snapshot.
export function isDarkModeApplied(): boolean {
  if (typeof document === 'undefined') return false;
  return document.documentElement.classList.contains('dark');
}

export function applyDarkMode(enabled = resolveDarkMode()): void {
  document.documentElement.classList.toggle('dark', enabled);
  document.documentElement.style.colorScheme = enabled ? 'dark' : 'light';
}

// React to OS theme changes while the preference is 'system' — the whole point
// of the "system" option is that the site tracks the OS live. One shared handler
// re-applies the class and notifies every subscriber; explicit light/dark ignore
// the OS.
function handleSystemThemeChange(): void {
  if (readThemePreference() !== 'system') return;
  applyDarkMode(resolveDarkMode('system'));
  listeners.forEach((listener) => listener());
}

export function applyInitialDarkMode(): void {
  applyDarkMode(resolveDarkMode());
  if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
    window
      .matchMedia('(prefers-color-scheme: dark)')
      .addEventListener('change', handleSystemThemeChange);
  }
}

// Persist a preference and apply it immediately. 'system' re-defers to the OS.
export function setThemePreference(preference: ThemePreference): void {
  applyDarkMode(resolveDarkMode(preference));
  setLegacyCookie(
    DARK_MODE_KEY,
    preference === 'dark' ? '1' : preference === 'light' ? '0' : 'system',
  );
  listeners.forEach((listener) => listener());
}

export function subscribeDarkMode(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useDarkMode(): boolean {
  return useSyncExternalStore(subscribeDarkMode, isDarkModeApplied, isDarkModeApplied);
}

export function useThemePreference(): ThemePreference {
  return useSyncExternalStore(subscribeDarkMode, readThemePreference, readThemePreference);
}

// Transitional compatibility shims for the still-legacy AdminLayout, written
// against the darkreader-era boolean API. Removed once the shell is rewritten as
// a shadcn island onto the token API above.
export function isDarkModeEnabled(): boolean {
  return isDarkModeApplied();
}

export function setDarkMode(enabled: boolean): void {
  setThemePreference(enabled ? 'dark' : 'light');
}
