import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDarkModeStore } from './dark-mode';

const syncThemeColorMeta = vi.fn();
const darkModeStore = createDarkModeStore(syncThemeColorMeta);
const {
  applyInitialDarkMode,
  isDarkModeApplied,
  readThemePreference,
  resolveDarkMode,
  setThemePreference,
} = darkModeStore;

// A controllable `(prefers-color-scheme: dark)` stub so tests that exercise the
// system preference can flip the OS theme live.
function mockMatchMedia(dark: boolean) {
  const changeListeners = new Set<() => void>();
  const mql = {
    matches: dark,
    media: '(prefers-color-scheme: dark)',
    addEventListener: (_type: string, listener: () => void) => changeListeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => changeListeners.delete(listener),
    dispatch(next: boolean) {
      mql.matches = next;
      changeListeners.forEach((listener) => listener());
    },
  };
  vi.stubGlobal('matchMedia', () => mql);
  return mql;
}

describe('dark mode preference store', () => {
  beforeEach(() => {
    syncThemeColorMeta.mockClear();
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    document.documentElement.style.colorScheme = '';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('reads the tri-state preference from the 1/0/system alphabet', () => {
    document.cookie = 'dark_mode=1;path=/';
    expect(readThemePreference()).toBe('dark');
    document.cookie = 'dark_mode=0;path=/';
    expect(readThemePreference()).toBe('light');
    document.cookie = 'dark_mode=system;path=/';
    expect(readThemePreference()).toBe('system');
    // The word alphabet is retired: nothing ever wrote it, so unknown values
    // fall back to following the OS.
    document.cookie = 'dark_mode=dark;path=/';
    expect(readThemePreference()).toBe('system');
  });

  it('defaults to following the system when no cookie is set', () => {
    expect(readThemePreference()).toBe('system');
  });

  it('resolves system to the OS preference and lets explicit choices override it', () => {
    mockMatchMedia(true);
    expect(resolveDarkMode('system')).toBe(true);
    expect(resolveDarkMode('light')).toBe(false);
    mockMatchMedia(false);
    expect(resolveDarkMode('system')).toBe(false);
    expect(resolveDarkMode('dark')).toBe(true);
  });

  it('reports the applied theme from the live class, not the cookie', () => {
    // The trigger icon and the toaster read this, so they must track the rendered
    // `.dark` class rather than the cookie. When the two drift (another tab, or a
    // stale legacy cookie at a different path) the UI still reflects what is on
    // screen instead of getting stuck.
    document.documentElement.classList.add('dark');
    document.cookie = 'dark_mode=0;path=/';
    expect(isDarkModeApplied()).toBe(true);

    document.documentElement.classList.remove('dark');
    document.cookie = 'dark_mode=1;path=/';
    expect(isDarkModeApplied()).toBe(false);
  });

  it('persists an explicit dark choice as the legacy 1 and applies the class', () => {
    setThemePreference('dark');

    expect(syncThemeColorMeta).toHaveBeenCalledOnce();
    expect(document.cookie).toContain('dark_mode=1');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('persists an explicit light choice as the legacy 0 and clears the class', () => {
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';

    setThemePreference('light');

    expect(document.cookie).toContain('dark_mode=0');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('persists system and re-defers to the OS preference', () => {
    mockMatchMedia(true);

    setThemePreference('system');

    expect(document.cookie).toContain('dark_mode=system');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('applies the OS preference on startup when following the system', () => {
    mockMatchMedia(true);

    applyInitialDarkMode();

    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('tracks live OS changes while following the system', () => {
    const media = mockMatchMedia(false);
    applyInitialDarkMode();
    expect(document.documentElement.classList.contains('dark')).toBe(false);

    media.dispatch(true);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe('dark');

    media.dispatch(false);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('ignores live OS changes once an explicit choice is made', () => {
    const media = mockMatchMedia(false);
    applyInitialDarkMode();

    setThemePreference('light');
    media.dispatch(true);

    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('treats a malformed cookie as system and follows a light OS', () => {
    mockMatchMedia(false);
    document.cookie = 'dark_mode=%E0%A4%A;path=/';

    expect(() => applyInitialDarkMode()).not.toThrow();
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });
});
