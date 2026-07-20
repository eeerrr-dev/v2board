import { createDarkModeStore, type ThemePreference } from '@v2board/ui/dark-mode';
import { syncAdminThemeColorMeta } from './runtime-config';

const darkModeStore = createDarkModeStore(syncAdminThemeColorMeta);

export const {
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
} = darkModeStore;

export type { ThemePreference };
