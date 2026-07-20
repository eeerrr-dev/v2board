import { createDarkModeStore, type ThemePreference } from '@v2board/ui/dark-mode';
import { syncRuntimeThemeColorMeta } from './runtime-config';

const darkModeStore = createDarkModeStore(syncRuntimeThemeColorMeta);

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
