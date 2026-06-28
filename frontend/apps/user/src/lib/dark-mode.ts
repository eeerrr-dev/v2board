import { useSyncExternalStore } from 'react';
import { getLegacyCookie, setLegacyCookie } from './legacy-cookie';

const DARK_MODE_KEY = 'dark_mode';
const listeners = new Set<() => void>();

export function isDarkModeEnabled(): boolean {
  return getLegacyCookie(DARK_MODE_KEY) === '1';
}

export function applyDarkMode(enabled = isDarkModeEnabled()): void {
  document.documentElement.classList.toggle('dark', enabled);
  document.documentElement.style.colorScheme = enabled ? 'dark' : 'light';
}

export function applyInitialDarkMode(): void {
  applyDarkMode(isDarkModeEnabled());
}

export function setDarkMode(enabled: boolean): void {
  applyDarkMode(enabled);
  setLegacyCookie(DARK_MODE_KEY, enabled ? 1 : 0);
  listeners.forEach((listener) => listener());
}

export function subscribeDarkMode(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useDarkMode(): boolean {
  return useSyncExternalStore(subscribeDarkMode, isDarkModeEnabled, isDarkModeEnabled);
}
