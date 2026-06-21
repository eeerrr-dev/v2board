import { disable as disableDarkReader, enable as enableDarkReader } from 'darkreader';

const DARK_MODE_KEY = 'dark_mode';
const LEGACY_DARK_READER_OPTIONS = {
  brightness: 100,
  contrast: 90,
  sepia: 10,
};

export function isDarkModeEnabled(): boolean {
  return getLegacyCookie(DARK_MODE_KEY) === '1';
}

export function applyDarkMode(enabled = isDarkModeEnabled()): void {
  if (enabled) {
    enableDarkReader(LEGACY_DARK_READER_OPTIONS);
  } else {
    disableDarkReader();
  }
}

export function applyInitialDarkMode(): void {
  if (isDarkModeEnabled()) {
    applyDarkMode(true);
  }
}

export function setDarkMode(enabled: boolean): void {
  applyDarkMode(enabled);
  setLegacyCookie(DARK_MODE_KEY, enabled ? 1 : 0);
}

function getLegacyCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    const [key, raw] = item.split('=');
    if (key !== name || raw === undefined) return value;
    try {
      return decodeURIComponent(raw);
    } catch {
      return value;
    }
  }, '');
}

function setLegacyCookie(name: string, value: string | number, minutes = 525600): void {
  const expires = new Date(Date.now() + minutes * 60_000).toUTCString();
  document.cookie = `${name}=${encodeURIComponent(value)};expires=${expires};path=/`;
}
