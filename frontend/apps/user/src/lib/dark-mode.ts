import { disable as disableDarkReader, enable as enableDarkReader } from 'darkreader';
import { getLegacyCookie, setLegacyCookie } from './legacy-cookie';

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
